//! The IPTC Core and Extension fields photographers fill in (title, caption, creator and
//! contact, rights, locations, event, persons shown, workflow fields): reading them from XMP
//! (sidecar or embedded) and IPTC-IIM, editing them, and writing them back.
//!
//! - A file's own fields follow the Metadata Working Group rule: XMP wins over IIM where both
//!   have a field ([`Embedded::record`]); IIM fills in what XMP lacks.
//! - A sidecar's fields win over the file's ([`Record::over`]). A field the sidecar holds
//!   empty (an empty element, as [`write_xmp`] writes for a cleared field) says "none" over
//!   the file's own.
//! - Writing changes only the properties of the fields edited; everything else in the packet is
//!   kept byte for byte. [`embed`] puts a packet (and an IIM block) into a JPEG or TIFF without
//!   touching any other byte of it.
//!
//! Location shown is written to both the Extension's `LocationShown` (its first entry) and the
//! legacy IPTC Core properties (`photoshop:City` …), which IIM and older programs read;
//! location created goes to `LocationCreated`. Each edits one location.

use std::collections::BTreeMap;

use crate::keywords::{self as kw, Vocabulary};
use crate::xmp::Xmp;

pub mod embed;
mod iim;
pub mod template;

pub use embed::{Embedded, read_embedded};
pub use iim::{iim_limit, read_iim, write_iim};

/// An editable field.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Field {
    Title,
    Headline,
    Description,
    Keywords,
    Creator,
    CreatorJobTitle,
    ContactAddress,
    ContactCity,
    ContactRegion,
    ContactPostalCode,
    ContactCountry,
    ContactPhone,
    ContactEmail,
    ContactUrl,
    Copyright,
    Credit,
    Source,
    UsageTerms,
    Instructions,
    JobId,
    ShownSublocation,
    ShownCity,
    ShownState,
    ShownCountry,
    ShownCountryCode,
    CreatedSublocation,
    CreatedCity,
    CreatedState,
    CreatedCountry,
    CreatedCountryCode,
    Event,
    PersonsShown,
}

/// The section a field is listed under.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Group {
    Description,
    Creator,
    Rights,
    Shown,
    Created,
    Workflow,
}

/// How a field's value is typed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// One line.
    Line,
    /// Text that may have several lines.
    Text,
    /// Several values (typed separated by `;`).
    List,
}

/// Where a field lives in XMP.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Prop {
    Simple(&'static str),
    LangAlt(&'static str),
    Seq(&'static str),
    Bag(&'static str),
    Keywords,
    /// A member of `Iptc4xmpCore:CreatorContactInfo`.
    Contact(&'static str),
    /// A member of the first `Iptc4xmpExt:LocationShown`, and its legacy property.
    Shown(&'static str, &'static str),
    /// A member of the first `Iptc4xmpExt:LocationCreated`.
    Created(&'static str),
}

struct Spec {
    field: Field,
    key: &'static str,
    group: Group,
    kind: Kind,
    prop: Prop,
}

const fn spec(field: Field, key: &'static str, group: Group, kind: Kind, prop: Prop) -> Spec {
    Spec {
        field,
        key,
        group,
        kind,
        prop,
    }
}

use Field as F;
use Group as G;
use Kind as K;

/// Every field, in the order the editor lists them.
const SPECS: &[Spec] = &[
    spec(
        F::Title,
        "title",
        G::Description,
        K::Line,
        Prop::LangAlt("dc:title"),
    ),
    spec(
        F::Headline,
        "headline",
        G::Description,
        K::Line,
        Prop::Simple("photoshop:Headline"),
    ),
    spec(
        F::Description,
        "description",
        G::Description,
        K::Text,
        Prop::LangAlt("dc:description"),
    ),
    spec(
        F::Keywords,
        "keywords",
        G::Description,
        K::List,
        Prop::Keywords,
    ),
    spec(
        F::PersonsShown,
        "persons-shown",
        G::Description,
        K::List,
        Prop::Bag("Iptc4xmpExt:PersonInImage"),
    ),
    spec(
        F::Event,
        "event",
        G::Description,
        K::Line,
        Prop::LangAlt("Iptc4xmpExt:Event"),
    ),
    spec(
        F::Creator,
        "creator",
        G::Creator,
        K::List,
        Prop::Seq("dc:creator"),
    ),
    spec(
        F::CreatorJobTitle,
        "creator-job-title",
        G::Creator,
        K::Line,
        Prop::Simple("photoshop:AuthorsPosition"),
    ),
    spec(
        F::ContactAddress,
        "contact-address",
        G::Creator,
        K::Line,
        Prop::Contact("CiAdrExtadr"),
    ),
    spec(
        F::ContactCity,
        "contact-city",
        G::Creator,
        K::Line,
        Prop::Contact("CiAdrCity"),
    ),
    spec(
        F::ContactRegion,
        "contact-region",
        G::Creator,
        K::Line,
        Prop::Contact("CiAdrRegion"),
    ),
    spec(
        F::ContactPostalCode,
        "contact-postal-code",
        G::Creator,
        K::Line,
        Prop::Contact("CiAdrPcode"),
    ),
    spec(
        F::ContactCountry,
        "contact-country",
        G::Creator,
        K::Line,
        Prop::Contact("CiAdrCtry"),
    ),
    spec(
        F::ContactPhone,
        "contact-phone",
        G::Creator,
        K::Line,
        Prop::Contact("CiTelWork"),
    ),
    spec(
        F::ContactEmail,
        "contact-email",
        G::Creator,
        K::Line,
        Prop::Contact("CiEmailWork"),
    ),
    spec(
        F::ContactUrl,
        "contact-url",
        G::Creator,
        K::Line,
        Prop::Contact("CiUrlWork"),
    ),
    spec(
        F::Copyright,
        "copyright",
        G::Rights,
        K::Line,
        Prop::LangAlt("dc:rights"),
    ),
    spec(
        F::Credit,
        "credit",
        G::Rights,
        K::Line,
        Prop::Simple("photoshop:Credit"),
    ),
    spec(
        F::Source,
        "source",
        G::Rights,
        K::Line,
        Prop::Simple("photoshop:Source"),
    ),
    spec(
        F::UsageTerms,
        "usage-terms",
        G::Rights,
        K::Text,
        Prop::LangAlt("xmpRights:UsageTerms"),
    ),
    spec(
        F::ShownSublocation,
        "shown-sublocation",
        G::Shown,
        K::Line,
        Prop::Shown("Sublocation", "Iptc4xmpCore:Location"),
    ),
    spec(
        F::ShownCity,
        "shown-city",
        G::Shown,
        K::Line,
        Prop::Shown("City", "photoshop:City"),
    ),
    spec(
        F::ShownState,
        "shown-state",
        G::Shown,
        K::Line,
        Prop::Shown("ProvinceState", "photoshop:State"),
    ),
    spec(
        F::ShownCountry,
        "shown-country",
        G::Shown,
        K::Line,
        Prop::Shown("CountryName", "photoshop:Country"),
    ),
    spec(
        F::ShownCountryCode,
        "shown-country-code",
        G::Shown,
        K::Line,
        Prop::Shown("CountryCode", "Iptc4xmpCore:CountryCode"),
    ),
    spec(
        F::CreatedSublocation,
        "created-sublocation",
        G::Created,
        K::Line,
        Prop::Created("Sublocation"),
    ),
    spec(
        F::CreatedCity,
        "created-city",
        G::Created,
        K::Line,
        Prop::Created("City"),
    ),
    spec(
        F::CreatedState,
        "created-state",
        G::Created,
        K::Line,
        Prop::Created("ProvinceState"),
    ),
    spec(
        F::CreatedCountry,
        "created-country",
        G::Created,
        K::Line,
        Prop::Created("CountryName"),
    ),
    spec(
        F::CreatedCountryCode,
        "created-country-code",
        G::Created,
        K::Line,
        Prop::Created("CountryCode"),
    ),
    spec(
        F::Instructions,
        "instructions",
        G::Workflow,
        K::Text,
        Prop::Simple("photoshop:Instructions"),
    ),
    spec(
        F::JobId,
        "job-id",
        G::Workflow,
        K::Line,
        Prop::Simple("photoshop:TransmissionReference"),
    ),
];

const CONTACT: &str = "Iptc4xmpCore:CreatorContactInfo";
const SHOWN: &str = "Iptc4xmpExt:LocationShown";
const CREATED: &str = "Iptc4xmpExt:LocationCreated";

/// The namespaces written.
const NAMESPACES: &[(&str, &str)] = &[
    ("dc", "http://purl.org/dc/elements/1.1/"),
    ("photoshop", "http://ns.adobe.com/photoshop/1.0/"),
    ("xmpRights", "http://ns.adobe.com/xap/1.0/rights/"),
    (
        "Iptc4xmpCore",
        "http://iptc.org/std/Iptc4xmpCore/1.0/xmlns/",
    ),
    ("Iptc4xmpExt", "http://iptc.org/std/Iptc4xmpExt/2008-02-29/"),
    ("lr", "http://ns.adobe.com/lightroom/1.0/"),
];

impl Field {
    /// Every field, in the editor's order.
    pub fn all() -> impl Iterator<Item = Field> {
        SPECS.iter().map(|s| s.field)
    }

    fn spec(self) -> &'static Spec {
        SPECS.iter().find(|s| s.field == self).unwrap_or(&SPECS[0])
    }

    /// Its stable name: in templates, the Library's text and string ids (`metaedit-field-…`).
    pub fn key(self) -> &'static str {
        self.spec().key
    }

    pub fn from_key(k: &str) -> Option<Field> {
        SPECS.iter().find(|s| s.key == k).map(|s| s.field)
    }

    pub fn group(self) -> Group {
        self.spec().group
    }

    pub fn kind(self) -> Kind {
        self.spec().kind
    }

    /// The XMP property it is read from (for a member of a structure, the structure's path
    /// and the member, as [`crate::xmp`] flattens them).
    pub fn xmp_path(self) -> String {
        match self.spec().prop {
            Prop::Simple(n) | Prop::LangAlt(n) | Prop::Seq(n) | Prop::Bag(n) => n.to_owned(),
            Prop::Keywords => "dc:subject".to_owned(),
            Prop::Contact(m) => format!("{CONTACT}/Iptc4xmpCore:{m}"),
            Prop::Shown(m, _) => format!("{SHOWN}/Iptc4xmpExt:{m}"),
            Prop::Created(m) => format!("{CREATED}/Iptc4xmpExt:{m}"),
        }
    }
}

/// Field values. A field that is there with no values was cleared (it says "none" over
/// another source); one that is not there is not known from this source.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Record {
    values: BTreeMap<Field, Vec<String>>,
}

impl Record {
    pub fn new() -> Record {
        Record::default()
    }

    /// The field's values (single fields have at most one); None when not known.
    pub fn get(&self, f: Field) -> Option<&[String]> {
        self.values.get(&f).map(Vec::as_slice)
    }

    /// The field's values, empty when none.
    pub fn values(&self, f: Field) -> &[String] {
        self.get(f).unwrap_or(&[])
    }

    /// The field as one text: a list's values joined with "; ".
    pub fn text(&self, f: Field) -> String {
        self.values(f).join("; ")
    }

    /// Sets the field (empty values or blank text: cleared). Values are trimmed, blank ones
    /// dropped; a single field keeps its first value.
    pub fn set(&mut self, f: Field, values: Vec<String>) {
        let mut v: Vec<String> = values
            .into_iter()
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .collect();
        if f.kind() != Kind::List {
            v.truncate(1);
        } else {
            let mut seen = std::collections::HashSet::new();
            v.retain(|x| seen.insert(x.to_lowercase()));
        }
        self.values.insert(f, v);
    }

    /// Forgets the field (not known).
    pub fn unset(&mut self, f: Field) {
        self.values.remove(&f);
    }

    /// The fields known.
    pub fn fields(&self) -> impl Iterator<Item = Field> + '_ {
        self.values.keys().copied()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Whether every known field is cleared (nothing to show).
    pub fn is_blank(&self) -> bool {
        self.values.values().all(Vec::is_empty)
    }

    /// This record's fields, then `base`'s for those it does not know.
    pub fn over(&self, base: &Record) -> Record {
        let mut out = base.clone();
        for (f, v) in &self.values {
            out.values.insert(*f, v.clone());
        }
        out
    }

    /// Lines `key=value` (one per value, line breaks as spaces) of the fields with values:
    /// what the Library keeps and search reads.
    pub fn to_lines(&self) -> String {
        let mut out = String::new();
        for (f, vs) in &self.values {
            if *f == Field::Keywords {
                continue;
            }
            for v in vs {
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(f.key());
                out.push('=');
                out.extend(
                    v.chars()
                        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c }),
                );
            }
        }
        out
    }
}

/// Splits a list field's text at `;` (values trimmed, blanks dropped).
pub fn split_list(text: &str) -> Vec<String> {
    text.split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

/// A field's typed text as values: a list split at `;` (keywords made paths), else the text.
pub fn parse_text(f: Field, text: &str) -> Vec<String> {
    match f.kind() {
        Kind::List if f == Field::Keywords => split_list(text)
            .iter()
            .filter_map(|k| kw::normalise(k))
            .collect(),
        Kind::List => split_list(text),
        _ => {
            let t = text.trim();
            if t.is_empty() {
                Vec::new()
            } else {
                vec![t.to_owned()]
            }
        }
    }
}

// ----- Reading -------------------------------------------------------------------------------

/// The fields of a parsed XMP packet that it has values for.
pub fn from_xmp(x: &Xmp) -> Record {
    let mut r = Record::new();
    let has_shown = x
        .properties
        .iter()
        .any(|p| p.name.starts_with(&format!("{SHOWN}/")));
    for s in SPECS {
        let v: Option<Vec<String>> = match s.prop {
            Prop::Keywords => kw::from_xmp(x),
            Prop::LangAlt(n) | Prop::Simple(n) => x.get(n).map(|v| vec![v.to_owned()]),
            Prop::Seq(n) | Prop::Bag(n) => x.get_all(n).map(<[String]>::to_vec),
            Prop::Shown(m, legacy) => {
                if has_shown {
                    x.get(&format!("{SHOWN}/Iptc4xmpExt:{m}"))
                        .map(|v| vec![v.to_owned()])
                } else {
                    x.get(legacy).map(|v| vec![v.to_owned()])
                }
            }
            Prop::Contact(_) | Prop::Created(_) => {
                x.get(&s.field.xmp_path()).map(|v| vec![v.to_owned()])
            }
        };
        if let Some(v) = v.filter(|v| !v.is_empty()) {
            r.set(s.field, v);
        }
    }
    r
}

/// The fields of an XMP packet's text: those with values, and those it holds empty (cleared:
/// `Some(&[])`). A structure that is there defines all its members.
pub fn from_xmp_text(text: &str) -> Record {
    let x = crate::xmp::parse(text.as_bytes());
    let mut r = from_xmp(&x);
    let has = |name: &str| kw::find_property(text, name).is_some();
    let (contact, shown, created) = (has(CONTACT), has(SHOWN), has(CREATED));
    for s in SPECS {
        if r.get(s.field).is_some() {
            continue;
        }
        let present = match s.prop {
            Prop::Keywords => kw::from_xmp_text(text, &|_| false).is_some(),
            Prop::Simple(n) | Prop::LangAlt(n) | Prop::Seq(n) | Prop::Bag(n) => has(n),
            Prop::Contact(_) => contact,
            Prop::Shown(_, legacy) => shown || has(legacy),
            Prop::Created(_) => created,
        };
        if present {
            r.set(s.field, Vec::new());
        }
    }
    r
}

// ----- Editing -------------------------------------------------------------------------------

/// What an edit does to the keywords.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeywordEdit {
    /// The list given, whatever was there.
    Replace(Vec<String>),
    /// These added and taken off; the others stay.
    Changes(Vec<kw::Change>),
}

/// Changes to some fields of each photo: only the fields named are written.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Edit {
    /// New values (empty: cleared). Keywords go in `keywords`.
    pub set: Vec<(Field, Vec<String>)>,
    pub keywords: Option<KeywordEdit>,
}

impl Edit {
    pub fn is_empty(&self) -> bool {
        self.set.is_empty() && self.keywords.is_none()
    }

    /// The fields it writes.
    pub fn fields(&self) -> Vec<Field> {
        let mut v: Vec<Field> = self.set.iter().map(|(f, _)| *f).collect();
        if self.keywords.is_some() {
            v.push(Field::Keywords);
        }
        v.sort();
        v.dedup();
        v
    }

    /// `r` after the edit.
    pub fn apply(&self, r: &Record) -> Record {
        let mut out = r.clone();
        for (f, v) in &self.set {
            if *f != Field::Keywords {
                out.set(*f, v.clone());
            }
        }
        match &self.keywords {
            Some(KeywordEdit::Replace(k)) => out.set(Field::Keywords, k.clone()),
            Some(KeywordEdit::Changes(c)) => {
                let now = kw::apply(r.values(Field::Keywords), c);
                out.set(Field::Keywords, now);
            }
            None => {}
        }
        out
    }
}

// ----- Writing XMP ---------------------------------------------------------------------------

/// An empty packet to add properties to.
fn empty_packet() -> String {
    "<?xpacket begin=\"\u{feff}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n\
     <x:xmpmeta xmlns:x=\"adobe:ns:meta/\" x:xmptk=\"Specular\">\n \
     <rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n  \
     <rdf:Description rdf:about=\"\">\n  \
     </rdf:Description>\n \
     </rdf:RDF>\n\
     </x:xmpmeta>\n\
     <?xpacket end=\"w\"?>\n"
        .to_owned()
}

/// `existing` (an XMP packet's text, None for a new one) with the properties of `fields` set
/// from `rec`. Everything else is kept byte for byte. A cleared field is written empty when
/// `clear_as_empty` (a sidecar saying "none" over the file's own), else left out.
pub fn write_xmp(
    existing: Option<&str>,
    rec: &Record,
    fields: &[Field],
    vocab: Option<&Vocabulary>,
    clear_as_empty: bool,
) -> String {
    let mut s = match existing.filter(|s| s.contains("<rdf:Description")) {
        Some(s) => s.to_owned(),
        None => empty_packet(),
    };
    let mut done_contact = false;
    let mut done_shown = false;
    let mut done_created = false;
    let mut used: Vec<&str> = Vec::new();
    for f in fields {
        match f.spec().prop {
            Prop::Keywords => {
                let k = rec.values(Field::Keywords);
                if k.is_empty() && !clear_as_empty {
                    s = remove_property(&s, "dc:subject");
                    s = remove_property(&s, "lr:hierarchicalSubject");
                } else {
                    s = kw::write_lists(Some(&s), &kw::subject(k, vocab), k);
                }
            }
            Prop::Simple(n) => {
                let v = rec.values(*f);
                let el = v.first().map(|v| format!("<{n}>{}</{n}>", kw::escape(v)));
                s = put(&s, n, el, clear_as_empty);
                used.push(prefix_of(n));
            }
            Prop::LangAlt(n) => {
                s = put_lang_alt(&s, n, rec.values(*f).first(), clear_as_empty);
                used.push(prefix_of(n));
            }
            Prop::Seq(n) | Prop::Bag(n) => {
                let v = rec.values(*f);
                let kind = if matches!(f.spec().prop, Prop::Seq(_)) {
                    "Seq"
                } else {
                    "Bag"
                };
                let el = (!v.is_empty()).then(|| list_element(n, kind, v));
                s = put(&s, n, el, clear_as_empty);
                used.push(prefix_of(n));
            }
            Prop::Contact(_) if !done_contact => {
                done_contact = true;
                let members = members(rec, |p| match p {
                    Prop::Contact(m) => Some(m),
                    _ => None,
                });
                let el = (!members.is_empty())
                    .then(|| struct_element(CONTACT, "Iptc4xmpCore", &members, "   "));
                s = put(&s, CONTACT, el, clear_as_empty);
                used.push("Iptc4xmpCore");
            }
            Prop::Shown(..) if !done_shown => {
                done_shown = true;
                let members = members(rec, |p| match p {
                    Prop::Shown(m, _) => Some(m),
                    _ => None,
                });
                let el = (!members.is_empty()).then(|| location_bag(SHOWN, &members));
                s = put(&s, SHOWN, el, clear_as_empty);
                // The legacy properties, each its own.
                for sp in SPECS {
                    if let Prop::Shown(_, legacy) = sp.prop {
                        let el = rec
                            .values(sp.field)
                            .first()
                            .map(|v| format!("<{legacy}>{}</{legacy}>", kw::escape(v)));
                        s = put(&s, legacy, el, clear_as_empty);
                        used.push(prefix_of(legacy));
                    }
                }
                used.push("Iptc4xmpExt");
            }
            Prop::Created(_) if !done_created => {
                done_created = true;
                let members = members(rec, |p| match p {
                    Prop::Created(m) => Some(m),
                    _ => None,
                });
                let el = (!members.is_empty()).then(|| location_bag(CREATED, &members));
                s = put(&s, CREATED, el, clear_as_empty);
                used.push("Iptc4xmpExt");
            }
            _ => {}
        }
    }
    for (p, uri) in NAMESPACES {
        if used.contains(p) {
            s = ensure_ns_first(s, p, uri);
        }
    }
    s
}

fn prefix_of(name: &str) -> &str {
    name.split_once(':').map_or(name, |(p, _)| p)
}

/// The (member, value) pairs of a structure's fields that have values.
fn members(
    rec: &Record,
    member: impl Fn(Prop) -> Option<&'static str>,
) -> Vec<(&'static str, String)> {
    SPECS
        .iter()
        .filter_map(|s| {
            let m = member(s.prop)?;
            let v = rec.values(s.field).first()?;
            Some((m, v.clone()))
        })
        .collect()
}

/// `<name>` holding a structure of `members` (in prefix `ns`).
fn struct_element(name: &str, ns: &str, members: &[(&str, String)], indent: &str) -> String {
    let mut s = format!("<{name} rdf:parseType=\"Resource\">\n");
    for (m, v) in members {
        s.push_str(&format!(
            "{indent} <{ns}:{m}>{}</{ns}:{m}>\n",
            kw::escape(v)
        ));
    }
    s.push_str(&format!("{indent}</{name}>"));
    s
}

/// A bag with one location structure.
fn location_bag(name: &str, members: &[(&str, String)]) -> String {
    let mut s = format!("<{name}>\n    <rdf:Bag>\n     <rdf:li rdf:parseType=\"Resource\">\n");
    for (m, v) in members {
        s.push_str(&format!(
            "      <Iptc4xmpExt:{m}>{}</Iptc4xmpExt:{m}>\n",
            kw::escape(v)
        ));
    }
    s.push_str(&format!("     </rdf:li>\n    </rdf:Bag>\n   </{name}>"));
    s
}

fn list_element(name: &str, kind: &str, values: &[String]) -> String {
    let mut s = format!("<{name}>\n    <rdf:{kind}>\n");
    for v in values {
        s.push_str(&format!("     <rdf:li>{}</rdf:li>\n", kw::escape(v)));
    }
    s.push_str(&format!("    </rdf:{kind}>\n   </{name}>"));
    s
}

/// `s` with property `name` replaced by `element`; None removes it (or, `clear_as_empty`,
/// leaves it as an empty element).
fn put(s: &str, name: &str, element: Option<String>, clear_as_empty: bool) -> String {
    let element = match element {
        Some(e) => e,
        None if clear_as_empty => format!("<{name}/>"),
        None => return remove_property(s, name),
    };
    match kw::find_property(s, name) {
        // An attribute leaves its tag; the element goes in as a new one.
        Some(r) if !s[r.clone()].trim_start().starts_with('<') => {
            let rest = format!("{}{}", &s[..r.start], &s[r.end..]);
            kw::insert_property(&rest, &element)
        }
        Some(r) => format!("{}{element}{}", &s[..r.start], &s[r.end..]),
        None => kw::insert_property(s, &element),
    }
}

/// A language alternative: the `x-default` entry replaced when there is one (other
/// languages kept), else the whole property.
fn put_lang_alt(s: &str, name: &str, value: Option<&String>, clear_as_empty: bool) -> String {
    if let (Some(v), Some(r)) = (value, kw::find_property(s, name)) {
        let prop = &s[r.clone()];
        if let Some(li) = prop.find("xml:lang=\"x-default\"")
            && let Some(open_end) = prop[li..].find('>').map(|e| li + e + 1)
            && !prop[..open_end].ends_with("/>")
            && let Some(close) = prop[open_end..].find("</rdf:li>").map(|c| open_end + c)
        {
            return format!(
                "{}{}{}",
                &s[..r.start + open_end],
                kw::escape(v),
                &s[r.start + close..]
            );
        }
    }
    let el = value.map(|v| {
        format!(
            "<{name}>\n    <rdf:Alt>\n     <rdf:li xml:lang=\"x-default\">{}</rdf:li>\n    </rdf:Alt>\n   </{name}>",
            kw::escape(v)
        )
    });
    put(s, name, el, clear_as_empty)
}

/// `s` without property `name` (each time it is there).
fn remove_property(s: &str, name: &str) -> String {
    let mut s = s.to_owned();
    for _ in 0..16 {
        let Some(r) = kw::find_property(&s, name) else {
            break;
        };
        // The line it was on goes too when it had nothing else.
        let start = s[..r.start].trim_end_matches([' ', '\t']).len();
        let start = if s[..start].ends_with('\n') && s[r.end..].starts_with('\n') {
            start - 1
        } else {
            r.start
        };
        s = format!("{}{}", &s[..start], &s[r.end..]);
    }
    s
}

/// `prefix` declared on the first `rdf:Description` (properties added go there).
fn ensure_ns_first(s: String, prefix: &str, uri: &str) -> String {
    let Some(at) = s.find("<rdf:Description") else {
        return s;
    };
    let Some(end) = kw::tag_end(&s, at) else {
        return s;
    };
    if s[at..end].contains(&format!("xmlns:{prefix}=")) {
        return s;
    }
    let after = at + "<rdf:Description".len();
    format!("{} xmlns:{prefix}=\"{uri}\"{}", &s[..after], &s[after..])
}

#[cfg(test)]
mod tests;
