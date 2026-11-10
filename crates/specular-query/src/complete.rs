//! Completion for the search field: what the text at the caret is (a field's value being
//! typed, or a word that could become a field name) and the edits that pick one value, toggle
//! several (ORed: `camera=a,b`), select a whole list or complete a field name.
//!
//! The text is scanned the way [`crate::parse`] reads it, but an unfinished term (an open
//! quote, a value not typed yet) still has a context: that is when completion is wanted.

use crate::facet::FacetField;
use crate::parse::{self, Field, Span};

/// A value of the term at the caret.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    /// The value without quotes.
    pub value: String,
    /// Where it is in the text (quotes included).
    pub span: Span,
}

/// A field term with the caret in its value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValueCtx {
    /// The whole term (`-` included).
    pub span: Span,
    pub negated: bool,
    pub field: Field,
    /// The facet whose values the picker lists.
    pub facet: FacetField,
    /// The field name as typed (`cam`), and the operator as typed (`:`, `!=`).
    pub name: String,
    pub op: String,
    /// Where the values start (after the operator).
    pub values_at: usize,
    /// The values other than the one at the caret.
    pub items: Vec<Item>,
    /// The value at the caret (empty right after the operator or a comma).
    pub partial: Item,
}

impl ValueCtx {
    /// Whether the term picks values (`:` and `=`, not a comparison) and can take several.
    pub fn plain(&self) -> bool {
        plain_op(&self.op) && self.facet.several()
    }

    /// The comparison the picker counts for (`iso>=`: each value counts the files at it and
    /// above; a ladder's `:` counts as `>=`); None for plain values and `!=`.
    pub fn compare(&self) -> Option<parse::Op> {
        let op = self.op.trim_start_matches(':');
        Some(match op {
            ">=" => parse::Op::Ge,
            ">" => parse::Op::Gt,
            "<=" => parse::Op::Le,
            "<" => parse::Op::Lt,
            "" | "=" if self.facet.ladder().is_some() => parse::Op::Ge,
            _ => return None,
        })
    }

    /// Whether `key` is among the term's values (other than the one being typed).
    pub fn has(&self, key: &str) -> bool {
        self.items.iter().any(|i| same(self.facet, &i.value, key))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Context {
    Value(ValueCtx),
    /// A bare word that starts a field name: `span` is the word (after any `-`).
    Name {
        span: Span,
        prefix: String,
    },
}

/// The facet a field's values come from (None: no list of values).
pub fn facet_of(field: Field) -> Option<FacetField> {
    match field {
        Field::Is | Field::Has => Some(FacetField::Flags),
        // Capture dates by month as well as year.
        Field::Date => Some(FacetField::Taken),
        f => FacetField::of(f),
    }
}

/// The field names a name completion offers, in order: (name, facet). `is` and `has` both
/// list every flag word.
pub const NAMES: [(&str, FacetField); 50] = [
    ("camera", FacetField::Camera),
    ("lens", FacetField::Lens),
    ("ext", FacetField::Format),
    ("rating", FacetField::Rating),
    ("label", FacetField::Label),
    ("date", FacetField::Taken),
    ("modified", FacetField::Modified),
    ("orientation", FacetField::Orientation),
    ("is", FacetField::Flags),
    ("has", FacetField::Flags),
    ("iso", FacetField::Iso),
    ("aperture", FacetField::Aperture),
    ("focal", FacetField::Focal),
    ("exposure", FacetField::Exposure),
    ("width", FacetField::Width),
    ("height", FacetField::Height),
    ("megapixels", FacetField::Megapixels),
    ("size", FacetField::Size),
    ("bits", FacetField::Bits),
    ("pages", FacetField::Pages),
    ("space", FacetField::Space),
    ("dupes", FacetField::Dupes),
    ("similar", FacetField::Similar),
    ("tag", FacetField::Tag),
    ("folder", FacetField::Folder),
    ("place", FacetField::Place),
    ("object", FacetField::Object),
    ("filter", FacetField::Filter),
    ("imagetyp", FacetField::FrameType),
    ("edited", FacetField::Edited),
    ("module", FacetField::Module),
    ("style", FacetField::Style),
    ("version", FacetField::Version),
    ("make", FacetField::Make),
    ("flash", FacetField::Flash),
    ("wb", FacetField::WhiteBalance),
    ("ev", FacetField::ExpComp),
    ("program", FacetField::Program),
    ("metering", FacetField::Metering),
    ("focal35", FacetField::Focal35),
    ("software", FacetField::Software),
    ("artist", FacetField::Artist),
    ("copyright", FacetField::Copyright),
    ("serial", FacetField::Serial),
    ("lensserial", FacetField::LensSerial),
    ("exif", FacetField::Exif),
    ("credit", FacetField::Credit),
    ("source", FacetField::Source),
    ("event", FacetField::Event),
    ("job", FacetField::JobId),
];

/// The names `prefix` could become: for each field, the first of its names (aliases
/// included) that starts with it, so `mod` offers `model`. A complete name is offered too.
pub fn names(prefix: &str) -> Vec<(&'static str, FacetField)> {
    let p = prefix.to_ascii_lowercase();
    if p.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<(&'static str, FacetField)> = Vec::new();
    for (name, facet) in NAMES {
        let field = Field::from_name(name).expect("a field");
        let alias = if name.starts_with(&p) {
            Some(name)
        } else {
            parse::FIELDS
                .iter()
                .find(|(n, f)| *f == field && n.starts_with(&p))
                .map(|(n, _)| *n)
        };
        if let Some(a) = alias {
            out.push((a, facet));
        }
    }
    out
}

/// The context at byte `caret` of `text`.
pub fn context(text: &str, caret: usize) -> Option<Context> {
    let caret = caret.min(text.len());
    let b = text.as_bytes();
    let mut i = 0;
    while i < text.len() {
        while i < text.len() && text[i..].starts_with(char::is_whitespace) {
            i += text[i..].chars().next().map_or(1, char::len_utf8);
        }
        if i >= text.len() || i > caret {
            return None;
        }
        let start = i;
        let mut negated = false;
        if b[i] == b'-' {
            negated = true;
            i += 1;
        }
        if i < text.len() && b[i] == b'"' {
            // A quoted phrase.
            i = parse::quoted_at(text, i).1;
            if caret <= i {
                return None;
            }
            continue;
        }
        let name_start = i;
        while i < text.len()
            && (b[i].is_ascii_alphabetic()
                || b[i] == b'_'
                || (i > name_start && b[i].is_ascii_digit()))
        {
            i += 1;
        }
        let name_end = i;
        let op_len = if name_end > name_start {
            op_at(&text[i..])
        } else {
            0
        };
        if op_len == 0 {
            // A plain word.
            i = name_start;
            while i < text.len() && !text[i..].starts_with(char::is_whitespace) {
                i += text[i..].chars().next().map_or(1, char::len_utf8);
            }
            if caret <= i {
                let word = &text[name_start..i];
                return (caret == i
                    && name_end == i
                    && !word.is_empty()
                    && !names(word).is_empty())
                .then(|| Context::Name {
                    span: Span {
                        start: name_start,
                        end: i,
                    },
                    prefix: word.to_owned(),
                });
            }
            continue;
        }
        let name = &text[name_start..name_end];
        let op = &text[name_end..name_end + op_len];
        i = name_end + op_len;
        let values_at = i;
        let mut items = Vec::new();
        loop {
            let s = i;
            let value = if i < text.len() && b[i] == b'"' {
                let (v, end, _) = parse::quoted_at(text, i);
                i = end;
                v
            } else {
                while i < text.len()
                    && !text[i..].starts_with(char::is_whitespace)
                    && b[i] != b','
                    && b[i] != b'"'
                {
                    i += text[i..].chars().next().map_or(1, char::len_utf8);
                }
                text[s..i].to_owned()
            };
            items.push(Item {
                value,
                span: Span { start: s, end: i },
            });
            if i < text.len() && b[i] == b',' {
                i += 1;
                if i < text.len() && !text[i..].starts_with(char::is_whitespace) {
                    continue;
                }
                // A trailing comma: an empty value after it.
                items.push(Item {
                    value: String::new(),
                    span: Span { start: i, end: i },
                });
            }
            break;
        }
        if caret > i {
            continue;
        }
        if caret < values_at {
            return None;
        }
        let field = Field::from_name(name)?;
        let facet = facet_of(field)?;
        let at = items
            .iter()
            .rposition(|it| it.span.start <= caret && caret <= it.span.end)?;
        let partial = items.remove(at);
        items.retain(|it| !it.value.trim().is_empty());
        return Some(Context::Value(ValueCtx {
            span: Span { start, end: i },
            negated,
            field,
            facet,
            name: name.to_owned(),
            op: op.to_owned(),
            values_at,
            items,
            partial,
        }));
    }
    None
}

/// The length of the operator at the start of `s` (0: none), as the parser reads it.
fn op_at(s: &str) -> usize {
    let colon = usize::from(s.starts_with(':'));
    let after = &s[colon..];
    for t in [">=", "<=", "!=", ">", "<", "="] {
        if after.starts_with(t) {
            return colon + t.len();
        }
    }
    colon
}

/// Whether `op` (as typed) picks values rather than compares.
fn plain_op(op: &str) -> bool {
    matches!(op, ":" | "=" | "!=" | ":=" | ":!=")
}

fn same(facet: FacetField, a: &str, b: &str) -> bool {
    crate::facet::same(facet, a, b)
}

/// The operator a picked value is selected with: a text field's `:` (contains, handy while
/// typing) becomes `=`, so the value means just that camera; anything else is kept.
fn op_for(c: &ValueCtx) -> String {
    let (_, op) = c.facet.term();
    if c.facet.ladder().is_some() && matches!(c.op.as_str(), ":" | "=" | ":=") {
        // A ladder's values are thresholds.
        ">=".to_owned()
    } else if c.op == ":" && op == parse::Op::Eq {
        "=".to_owned()
    } else {
        c.op.clone()
    }
}

/// The term rebuilt with `items` (as typed) and `keys` (quoted as needed), and a trailing
/// comma when `more` (the caret then sits on an empty value, ready for the next).
fn rebuild(text: &str, c: &ValueCtx, keep: &[&Item], keys: &[&str], more: bool) -> String {
    let mut vals: Vec<String> = keep
        .iter()
        .map(|i| text[i.span.start..i.span.end].to_owned())
        .collect();
    vals.extend(keys.iter().map(|k| parse::quote(k)));
    let mut term = format!(
        "{}{}{}{}",
        if c.negated { "-" } else { "" },
        c.name,
        op_for(c),
        vals.join(",")
    );
    if more && !vals.is_empty() {
        term.push(',');
    }
    format!("{}{term}{}", &text[..c.span.start], &text[c.span.end..])
}

/// The text with the value being typed replaced by `key` (kept once if already there). The
/// caret goes to the end of the term.
pub fn pick(text: &str, c: &ValueCtx, key: &str) -> (String, usize) {
    // A comparison takes one value: the picked one.
    let keep: Vec<&Item> = if c.plain() {
        c.items.iter().collect()
    } else {
        Vec::new()
    };
    let keys: Vec<&str> = if c.plain() && c.has(key) {
        Vec::new()
    } else {
        vec![key]
    };
    let out = rebuild(text, c, &keep, &keys, false);
    let end = out.len() - (text.len() - c.span.end);
    (out, end)
}

/// The text with `key` added to the term's values or, when it is one, removed; the value
/// being typed goes (it was a filter for the list). A comparison just picks.
pub fn toggle(text: &str, c: &ValueCtx, key: &str) -> (String, usize) {
    if !c.plain() {
        return pick(text, c, key);
    }
    let keep: Vec<&Item> = c
        .items
        .iter()
        .filter(|i| !same(c.facet, &i.value, key))
        .collect();
    let keys: Vec<&str> = if c.has(key) { Vec::new() } else { vec![key] };
    let out = rebuild(text, c, &keep, &keys, true);
    let end = out.len() - (text.len() - c.span.end);
    (out, end)
}

/// The text with every one of `keys` among the term's values.
pub fn select_all(text: &str, c: &ValueCtx, keys: &[&str]) -> (String, usize) {
    if !c.plain() {
        return (text.to_owned(), c.span.end);
    }
    let keep: Vec<&Item> = c.items.iter().collect();
    let mut add: Vec<&str> = Vec::new();
    for k in keys {
        if !c.has(k) && !add.iter().any(|a| same(c.facet, a, k)) {
            add.push(k);
        }
    }
    let out = rebuild(text, c, &keep, &add, true);
    let end = out.len() - (text.len() - c.span.end);
    (out, end)
}

/// The text with the term's operator changed to `op` (`:`, `>=`, `<=`, …), keeping the value
/// being typed (and the other values, between plain operators). The caret goes to the end
/// of the term.
pub fn set_op(text: &str, c: &ValueCtx, op: &str) -> (String, usize) {
    let mut vals: Vec<&str> = Vec::new();
    if plain_op(&c.op) && plain_op(op) {
        vals.extend(c.items.iter().map(|i| &text[i.span.start..i.span.end]));
    }
    let partial = &text[c.partial.span.start..c.partial.span.end];
    if !partial.trim().is_empty() {
        vals.push(partial);
    }
    let term = format!(
        "{}{}{op}{}",
        if c.negated { "-" } else { "" },
        c.name,
        vals.join(",")
    );
    let out = format!("{}{term}{}", &text[..c.span.start], &text[c.span.end..]);
    let end = c.span.start + term.len();
    (out, end)
}

/// The text with the term's trailing comma (left by a toggle) gone.
pub fn tidy(text: &str, c: &ValueCtx) -> String {
    if c.partial.value.is_empty()
        && c.partial.span.start > c.values_at
        && text[..c.partial.span.start].ends_with(',')
    {
        let at = c.partial.span.start - 1;
        return format!("{}{}", &text[..at], &text[at + 1..]);
    }
    text.to_owned()
}

/// The text with the word at `span` completed to `name:`.
pub fn complete_name(text: &str, span: Span, name: &str) -> (String, usize) {
    let out = format!("{}{name}:{}", &text[..span.start], &text[span.end..]);
    (out, span.start + name.len() + 1)
}
