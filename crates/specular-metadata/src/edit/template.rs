//! Metadata templates: named sets of field values (copyright and creator, say) applied to
//! photos in one step, kept as text.
//!
//! The text has one template per `[name]` line, then `key = value` lines ([`Field::key`]); a
//! list field repeats its key once per value, a line break in a value is `\n` and a backslash
//! `\\`. `keywords-mode = add` or `replace` says what the template's keywords do to the
//! photos' own. Lines starting with `#` are comments; unknown keys are kept as they are read
//! (and dropped on write).

use super::{Edit, Field, KeywordEdit, Kind};
use crate::keywords as kw;

/// What a template's keywords do.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum KeywordMode {
    /// Added to the photos' own.
    #[default]
    Add,
    /// In place of the photos' own.
    Replace,
}

/// One template.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Template {
    pub name: String,
    /// The fields it sets, in the editor's order, each with its values.
    pub fields: Vec<(Field, Vec<String>)>,
    pub keyword_mode: KeywordMode,
}

impl Template {
    pub fn new(name: &str) -> Template {
        Template {
            name: name.trim().to_owned(),
            ..Template::default()
        }
    }

    /// The values it sets for `f` (None: it leaves `f` alone).
    pub fn get(&self, f: Field) -> Option<&[String]> {
        self.fields
            .iter()
            .find(|(g, _)| *g == f)
            .map(|(_, v)| v.as_slice())
    }

    /// Sets `f` (no values: the template leaves it alone).
    pub fn set(&mut self, f: Field, values: Vec<String>) {
        self.fields.retain(|(g, _)| *g != f);
        let v: Vec<String> = values
            .into_iter()
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .collect();
        if !v.is_empty() {
            self.fields.push((f, v));
        }
        let order: Vec<Field> = Field::all().collect();
        self.fields
            .sort_by_key(|(g, _)| order.iter().position(|o| o == g));
    }

    /// The edit that applies it.
    pub fn edit(&self) -> Edit {
        let mut e = Edit::default();
        for (f, v) in &self.fields {
            if *f == Field::Keywords {
                e.keywords = Some(match self.keyword_mode {
                    KeywordMode::Replace => KeywordEdit::Replace(v.clone()),
                    KeywordMode::Add => {
                        KeywordEdit::Changes(v.iter().map(|k| kw::Change::Add(k.clone())).collect())
                    }
                });
            } else {
                e.set.push((*f, v.clone()));
            }
        }
        e
    }
}

fn escape(v: &str) -> String {
    v.replace('\\', "\\\\")
        .replace('\r', "")
        .replace('\n', "\\n")
}

fn unescape(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    let mut it = v.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('n') => out.push('\n'),
            Some(o) => out.push(o),
            None => out.push('\\'),
        }
    }
    out
}

/// The templates in `text`.
pub fn parse(text: &str) -> Vec<Template> {
    let mut out: Vec<Template> = Vec::new();
    for line in text.lines() {
        let line = line.trim_start_matches('\u{feff}').trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            out.push(Template::new(&unescape(name)));
            continue;
        }
        let Some(t) = out.last_mut() else { continue };
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let (k, v) = (k.trim(), unescape(v.trim()));
        if k == "keywords-mode" {
            t.keyword_mode = if v.eq_ignore_ascii_case("replace") {
                KeywordMode::Replace
            } else {
                KeywordMode::Add
            };
            continue;
        }
        let Some(f) = Field::from_key(k) else {
            continue;
        };
        let mut values = t.get(f).map(<[String]>::to_vec).unwrap_or_default();
        let v = if f == Field::Keywords {
            kw::normalise(&v).unwrap_or_default()
        } else {
            v
        };
        if f.kind() == Kind::List || values.is_empty() {
            values.push(v);
        }
        t.set(f, values);
    }
    out.retain(|t| !t.name.is_empty());
    out
}

/// `templates` as text ([`parse`] reads it back).
pub fn to_text(templates: &[Template]) -> String {
    let mut s = String::from("# Specular metadata templates\n");
    for t in templates {
        s.push_str(&format!("\n[{}]\n", escape(&t.name)));
        if t.get(Field::Keywords).is_some() {
            let mode = match t.keyword_mode {
                KeywordMode::Add => "add",
                KeywordMode::Replace => "replace",
            };
            s.push_str(&format!("keywords-mode = {mode}\n"));
        }
        for (f, vs) in &t.fields {
            for v in vs {
                s.push_str(&format!("{} = {}\n", f.key(), escape(v)));
            }
        }
    }
    s
}
