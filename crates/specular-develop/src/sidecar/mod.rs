//! The Develop sidecar: `<file>.<ext>.specular.xmp` next to
//! the image (`<file>.<ext>.vN.specular.xmp` for version N), written by default, read back
//! on open. The format is frozen and written down in `docs/sidecar-format.md`;
//! `tests/fixtures/sidecar/` holds a file of every module kind.
//!
//! - Module and field names are the registry's, never translated. A kind this build does not
//!   know is written `kind_<number>` and kept.
//! - Values: `True`/`False`, integers, floats in their shortest round-trip form (always with
//!   a `.` or an exponent), text, float lists separated by spaces. On reading, a field's type
//!   comes from the module's defaults; other fields are read by their form.
//! - Masks (`spd:masks`) and a module's blend (`spd:blend`) are readable ([`masks`]); brush
//!   strokes and the history (`spd:history`) are base64 of the catalogue's codec. Snapshots
//!   are readable module lists with their masks.
//! - **Forward compatible:** modules, fields, attributes and elements this build does not
//!   know are written back unchanged ([`to_xmp_over`]), and so is everything outside
//!   Develop's own `rdf:Description`.
//! - A load that migrates module params, or skips unknown modules, starts a fresh history.

pub(crate) mod b64;
pub mod files;
pub mod masks;
mod raw;

use std::io::Write as _;
use std::path::{Path, PathBuf};

use crate::model::codec::{self, DecodeError};
use crate::model::hash::{edit_hash, hex};
use crate::model::migrate::MigrationReport;
use crate::model::{
    Edit, Fields, History, ModuleInstance, ModuleKind, Params, Registry, SourceKey, Value,
};

/// The namespace of Develop's sidecar fields.
pub const NS: &str = "http://ns.specular.app/develop/1/";
const RDF: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#";
/// What follows the image's own name.
pub const SUFFIX: &str = ".specular.xmp";

/// The sidecar of `file` (`IMG_1234.CR2` → `IMG_1234.CR2.specular.xmp`).
pub fn path_for(file: &Path) -> PathBuf {
    let mut s = file.as_os_str().to_owned();
    s.push(SUFFIX);
    PathBuf::from(s)
}

/// Why a sidecar could not be read.
#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    /// Not XML, or not a Specular sidecar.
    Format(String),
    /// A base64 field does not decode.
    Codec(DecodeError),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(e) => write!(f, "{e}"),
            Error::Format(m) => write!(f, "not a Specular edit: {m}"),
            Error::Codec(e) => write!(f, "damaged edit data: {e:?}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<DecodeError> for Error {
    fn from(e: DecodeError) -> Self {
        Error::Codec(e)
    }
}

/// A sidecar read back.
#[derive(Debug)]
pub struct Loaded {
    pub edit: Edit,
    /// Empty unless module params were migrated (show its notices).
    pub report: MigrationReport,
    /// The history; empty when the sidecar had none or the load migrated modules.
    pub history: History,
    /// The version's (or style's) name, when it has one.
    pub name: Option<String>,
    /// What wrote it (`Specular 0.1.0`), when it says.
    pub writer: Option<String>,
    /// Modules this build does not know, by name: not run, kept in the file.
    pub unknown_modules: Vec<String>,
    /// Made with a newer process version than this build renders: it may look different.
    pub newer_process: bool,
    /// A mask shape or a blend could not be read (a newer Specular's): left out of `edit`
    /// (a module whose blend it was is off). The file is then never overwritten.
    pub partial: bool,
}

/// Escapes text for an XML attribute or element.
fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\n' => o.push_str("&#10;"),
            '\r' => o.push_str("&#13;"),
            '\t' => o.push_str("&#9;"),
            c => o.push(c),
        }
    }
    o
}

fn float(v: f32) -> String {
    // Debug is the shortest form that reads back exactly, with a `.0` on whole numbers.
    format!("{v:?}")
}

/// A value's text form.
pub fn value_text(v: &Value) -> String {
    match v {
        Value::Bool(b) => if *b { "True" } else { "False" }.to_owned(),
        Value::Int(i) => i.to_string(),
        Value::Float(f) => float(*f),
        Value::Text(t) => t.clone(),
        Value::Floats(fs) => fs.iter().map(|f| float(*f)).collect::<Vec<_>>().join(" "),
    }
}

/// Reads a value's text form as the type of `like`, or by its form.
pub fn parse_value(s: &str, like: Option<&Value>) -> Value {
    let floats = |s: &str| -> Option<Vec<f32>> {
        s.split_ascii_whitespace()
            .map(|t| t.parse::<f32>().ok())
            .collect()
    };
    match like {
        Some(Value::Bool(_)) if matches!(s, "True" | "False") => return Value::Bool(s == "True"),
        Some(Value::Int(_)) => {
            if let Ok(i) = s.parse() {
                return Value::Int(i);
            }
        }
        Some(Value::Float(_)) => {
            if let Ok(f) = s.parse() {
                return Value::Float(f);
            }
        }
        Some(Value::Text(_)) => return Value::Text(s.to_owned()),
        Some(Value::Floats(_)) => {
            if let Some(v) = floats(s) {
                return Value::Floats(v);
            }
        }
        _ => {}
    }
    match s {
        "True" => return Value::Bool(true),
        "False" => return Value::Bool(false),
        _ => {}
    }
    if let Ok(i) = s.parse::<i64>() {
        return Value::Int(i);
    }
    if !s.contains(' ')
        && let Ok(f) = s.parse::<f32>()
    {
        return Value::Float(f);
    }
    if s.contains(' ')
        && let Some(v) = floats(s)
    {
        return Value::Floats(v);
    }
    Value::Text(s.to_owned())
}

fn module_name(reg: &Registry, k: ModuleKind) -> String {
    reg.get(k)
        .map_or_else(|| format!("kind_{}", k.0), |s| s.name.to_owned())
}

fn module_kind(reg: &Registry, name: &str) -> Option<ModuleKind> {
    if let Some(s) = reg.by_name(name) {
        return Some(s.kind);
    }
    name.strip_prefix("kind_")?.parse().ok().map(ModuleKind)
}

fn source_text(s: &SourceKey) -> String {
    format!(
        "size={} mtime={} hash={}",
        s.size,
        s.mtime_ns,
        hex(&s.head_tail)
    )
}

fn parse_source(t: &str) -> Option<SourceKey> {
    let mut k = SourceKey::default();
    for part in t.split_ascii_whitespace() {
        let (n, v) = part.split_once('=')?;
        match n {
            "size" => k.size = v.parse().ok()?,
            "mtime" => k.mtime_ns = v.parse().ok()?,
            "hash" => {
                if v.len() != 64 {
                    return None;
                }
                for (i, b) in k.head_tail.iter_mut().enumerate() {
                    *b = u8::from_str_radix(v.get(i * 2..i * 2 + 2)?, 16).ok()?;
                }
            }
            _ => {}
        }
    }
    Some(k)
}

/// What wrote the sidecars this build writes (`spd:writer`).
pub const WRITER: &str = concat!("Specular ", env!("CARGO_PKG_VERSION"));

/// The description attributes this build owns; others are written back as they were.
const OWN_ATTRS: [&str; 9] = [
    "schema",
    "process",
    "writer",
    "source",
    "customOrder",
    "editHash",
    "name",
    "masks",
    "snapshots",
];
/// The description elements this build owns.
const OWN_ELEMENTS: [&str; 4] = ["masks", "modules", "snapshots", "history"];
/// A module item's attributes this build owns.
const OWN_LI: [&str; 6] = ["module", "version", "instance", "enabled", "label", "blend"];

/// The sidecar text of `edit` (and its history, when given).
pub fn to_xmp(edit: &Edit, history: Option<&History>) -> String {
    to_xmp_named(edit, history, None)
}

/// [`to_xmp`] with a name (`spd:name`): a version's or a style's.
pub fn to_xmp_named(edit: &Edit, history: Option<&History>, name: Option<&str>) -> String {
    to_xmp_over(None, edit, history, name)
}

fn parse_doc(text: &str) -> Result<roxmltree::Document<'_>, roxmltree::Error> {
    let opts = roxmltree::ParsingOptions {
        allow_dtd: false,
        ..Default::default()
    };
    roxmltree::Document::parse_with_options(text, opts)
}

fn our_description<'a, 'i>(doc: &'a roxmltree::Document<'i>) -> Option<roxmltree::Node<'a, 'i>> {
    doc.descendants()
        .find(|n| n.has_tag_name((RDF, "Description")) && n.attribute((NS, "schema")).is_some())
}

/// [`to_xmp_named`] written over `existing` (the sidecar's text now): only Develop's own
/// description is replaced. Everything else in the file, and whatever that description holds
/// that this build does not own (other tools' attributes and elements, a newer Specular's
/// fields, modules and elements), is written back byte for byte.
pub fn to_xmp_over(
    existing: Option<&str>,
    edit: &Edit,
    history: Option<&History>,
    name: Option<&str>,
) -> String {
    if let Some(text) = existing
        && let Ok(doc) = parse_doc(text)
    {
        if let Some(d) = our_description(&doc) {
            let r = d.range();
            return format!(
                "{}{}{}",
                &text[..r.start],
                description(edit, history, name, Some((text, d))),
                &text[r.end..]
            );
        }
        if let Some(rdf) = doc.descendants().find(|n| n.has_tag_name((RDF, "RDF"))) {
            let r = rdf.range();
            if let Some(close) = text[r.clone()].rfind("</") {
                let at = r.start + close;
                return format!(
                    "{}  {}\n {}",
                    &text[..at],
                    description(edit, history, name, None),
                    &text[at..]
                );
            }
        }
    }
    let mut o = String::new();
    o.push_str("<?xpacket begin=\"\u{feff}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n");
    o.push_str("<x:xmpmeta xmlns:x=\"adobe:ns:meta/\">\n");
    o.push_str(&format!(" <rdf:RDF xmlns:rdf=\"{RDF}\">\n  "));
    o.push_str(&description(edit, history, name, None));
    o.push_str("\n </rdf:RDF>\n</x:xmpmeta>\n<?xpacket end=\"w\"?>\n");
    o
}

type Old<'a, 'i> = Option<(&'i str, roxmltree::Node<'a, 'i>)>;

/// Develop's `rdf:Description`, from its `<` to its `>`, over `old` (the one in the file).
fn description(edit: &Edit, history: Option<&History>, name: Option<&str>, old: Old) -> String {
    let reg = Registry::builtin();
    let mut o = String::new();
    o.push_str(&format!(
        "<rdf:Description rdf:about=\"\" xmlns:spd=\"{NS}\""
    ));
    if let Some((text, d)) = old {
        for decl in raw::xmlns_decls(raw::start_tag(text, d)) {
            if !decl.starts_with("xmlns:spd=") {
                o.push_str("\n    ");
                o.push_str(decl);
            }
        }
    }
    let attr = |o: &mut String, n: &str, v: &str| {
        o.push_str(&format!("\n    spd:{n}=\"{}\"", esc(v)));
    };
    attr(&mut o, "schema", &edit.schema.to_string());
    attr(&mut o, "process", &edit.process.to_string());
    attr(&mut o, "writer", WRITER);
    attr(&mut o, "source", &source_text(&edit.source));
    attr(
        &mut o,
        "customOrder",
        &value_text(&Value::Bool(edit.custom_order)),
    );
    attr(&mut o, "editHash", &hex(&edit_hash(edit)));
    if let Some(n) = name {
        attr(&mut o, "name", n);
    }
    if let Some((text, d)) = old {
        for a in d.attributes() {
            let own = a.namespace() == Some(NS) && OWN_ATTRS.contains(&a.name());
            if !own && !(a.namespace() == Some(RDF) && a.name() == "about") {
                o.push_str("\n    ");
                o.push_str(&text[a.range()]);
            }
        }
    }
    o.push_str(">\n");
    if !edit.masks.is_empty() {
        masks::masks_xml(&mut o, &edit.masks, 3);
    }
    o.push_str("   <spd:modules>\n");
    let old_seq = old.and_then(|(t, d)| Some((t, child(d, "modules")?)));
    modules_seq(&mut o, reg, &edit.modules, old_seq, 4);
    o.push_str("   </spd:modules>\n");
    if !edit.snapshots.is_empty() {
        let old_snaps: Vec<_> = old
            .and_then(|(t, d)| Some((t, child(d, "snapshots")?)))
            .map(|(t, s)| module_items(s).into_iter().map(|n| (t, n)).collect())
            .unwrap_or_default();
        o.push_str("   <spd:snapshots>\n    <rdf:Seq>\n");
        for (i, s) in edit.snapshots.iter().enumerate() {
            o.push_str(&format!("     <rdf:li spd:name=\"{}\">\n", esc(&s.name)));
            if !s.masks.is_empty() {
                masks::masks_xml(&mut o, &s.masks, 6);
            }
            o.push_str("      <spd:modules>\n");
            // The same snapshot in the file (same place, same name) lends its modules' extras.
            let old_mods = old_snaps
                .get(i)
                .filter(|(_, n)| n.attribute((NS, "name")) == Some(s.name.as_str()))
                .and_then(|&(t, n)| Some((t, child(n, "modules")?)));
            modules_seq(&mut o, reg, &s.modules, old_mods, 7);
            o.push_str("      </spd:modules>\n     </rdf:li>\n");
        }
        o.push_str("    </rdf:Seq>\n   </spd:snapshots>\n");
    }
    if let Some(h) = history
        && !h.steps.is_empty()
    {
        o.push_str(&format!(
            "   <spd:history>{}</spd:history>\n",
            b64::encode(&codec::encode_history(h))
        ));
    }
    if let Some((text, d)) = old {
        for c in raw::elements(d) {
            if !(c.tag_name().namespace() == Some(NS)
                && OWN_ELEMENTS.contains(&c.tag_name().name()))
            {
                o.push_str("   ");
                o.push_str(&text[c.range()]);
                o.push('\n');
            }
        }
    }
    o.push_str("  </rdf:Description>");
    o
}

/// The `spd:` element `name` under `n`.
fn child<'a, 'i>(n: roxmltree::Node<'a, 'i>, name: &str) -> Option<roxmltree::Node<'a, 'i>> {
    raw::elements(n).find(|c| c.has_tag_name((NS, name)))
}

/// The items of the `rdf:Seq` under `modules` (a `spd:modules` or `spd:snapshots`), in order.
fn module_items<'a, 'i>(modules: roxmltree::Node<'a, 'i>) -> Vec<roxmltree::Node<'a, 'i>> {
    raw::elements(modules)
        .filter(|n| n.has_tag_name((RDF, "Seq")))
        .flat_map(raw::elements)
        .filter(|n| n.has_tag_name((RDF, "li")))
        .collect()
}

/// A module list as an `rdf:Seq`, indented by `ind`. Over `old` (the list in the file), the
/// modules this build does not know keep their place after the module they followed, and
/// a module's fields, attributes and elements this build does not own are kept.
fn modules_seq(o: &mut String, reg: &Registry, modules: &[ModuleInstance], old: Old, ind: usize) {
    let pad = " ".repeat(ind);
    let items = old.map(|(t, m)| (t, module_items(m)));
    // Known items by (name, instance); unknown ones by the known item before them.
    let mut known: Vec<(String, u16, roxmltree::Node)> = Vec::new();
    let mut unknown: Vec<(Option<(String, u16)>, &str)> = Vec::new();
    if let Some((text, items)) = &items {
        let mut last: Option<(String, u16)> = None;
        for li in items {
            let name = li.attribute((NS, "module")).unwrap_or("");
            let inst = li
                .attribute((NS, "instance"))
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            if module_kind(reg, name).is_some() {
                known.push((name.to_owned(), inst, *li));
                last = Some((name.to_owned(), inst));
            } else {
                unknown.push((last.clone(), &text[li.range()]));
            }
        }
    }
    let text = items.as_ref().map_or("", |(t, _)| *t);
    let mut put_unknown = |o: &mut String, after: Option<&(String, u16)>| {
        unknown.retain(|(a, raw)| {
            if a.as_ref() == after {
                o.push_str(&format!("{pad} {raw}\n"));
                false
            } else {
                true
            }
        });
    };
    o.push_str(&format!("{pad}<rdf:Seq>\n"));
    put_unknown(o, None);
    for m in modules {
        let name = module_name(reg, m.kind);
        let old_li = known
            .iter()
            .find(|(n, i, _)| *n == name && *i == m.instance)
            .map(|k| k.2);
        module_li(o, &name, m, &pad, old_li.map(|n| (text, n)));
        put_unknown(o, Some(&(name, m.instance)));
    }
    // Those whose module is gone go last.
    for (_, raw) in unknown.drain(..) {
        o.push_str(&format!("{pad} {raw}\n"));
    }
    o.push_str(&format!("{pad}</rdf:Seq>\n"));
}

fn module_li(o: &mut String, name: &str, m: &ModuleInstance, pad: &str, old: Old) {
    o.push_str(&format!(
        "{pad} <rdf:li spd:module=\"{}\" spd:version=\"{}\" spd:instance=\"{}\" spd:enabled=\"{}\"",
        esc(name),
        m.version,
        m.instance,
        value_text(&Value::Bool(m.enabled)),
    ));
    if let Some(l) = &m.label {
        o.push_str(&format!(" spd:label=\"{}\"", esc(l)));
    }
    if let Some((text, li)) = old {
        for a in li.attributes() {
            if !(a.namespace() == Some(NS) && OWN_LI.contains(&a.name())) {
                o.push(' ');
                o.push_str(&text[a.range()]);
            }
        }
    }
    o.push_str(&format!(">\n{pad}  <spd:params"));
    let fields = m.params.to_fields();
    for (n, v) in &fields.0 {
        o.push_str(&format!("\n{pad}    spd:{n}=\"{}\"", esc(&value_text(v))));
    }
    if let Some((text, li)) = old {
        // A newer Specular's fields (an item of this version or later) stay.
        let old_version: u16 = li
            .attribute((NS, "version"))
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        if let Some(p) = child(li, "params") {
            for a in p.attributes() {
                let ours = a.namespace() == Some(NS);
                if !ours || (old_version >= m.version && fields.get(a.name()).is_none()) {
                    o.push_str(&format!("\n{pad}    {}", &text[a.range()]));
                }
            }
        }
    }
    o.push_str("/>\n");
    if let Some(b) = &m.blend {
        masks::blend_xml(o, b, Registry::builtin(), &format!("{pad}  "));
    }
    if let Some((text, li)) = old {
        for c in raw::elements(li) {
            if !c.has_tag_name((NS, "params")) && !c.has_tag_name((NS, "blend")) {
                o.push_str(&format!("{pad}  {}\n", &text[c.range()]));
            }
        }
    }
    o.push_str(&format!("{pad} </rdf:li>\n"));
}

fn bad(m: &str) -> Error {
    Error::Format(m.to_owned())
}

/// Reads sidecar text.
pub fn from_xmp(text: &str, reg: &Registry) -> Result<Loaded, Error> {
    let doc = parse_doc(text).map_err(|e| Error::Format(e.to_string()))?;
    let desc = our_description(&doc).ok_or_else(|| bad("no Develop description"))?;
    let num = |n: roxmltree::Node, a: &str| -> Option<u16> { n.attribute((NS, a))?.parse().ok() };
    let mut edit = Edit::empty(
        desc.attribute((NS, "source"))
            .and_then(parse_source)
            .unwrap_or_default(),
    );
    edit.process = num(desc, "process").ok_or_else(|| bad("no process version"))?;
    edit.custom_order = desc.attribute((NS, "customOrder")) == Some("True");
    let mut report = MigrationReport::default();
    let mut unknown = Vec::new();
    let mut partial = false;
    // Older sidecars: base64 of the codec.
    if let Some(s) = desc.attribute((NS, "masks")) {
        edit.masks = blob(s, reg)?.0.masks;
    }
    if let Some(m) = child(desc, "masks") {
        edit.masks = masks::read_masks(m, reg, &mut partial);
    }
    // Older sidecars wrote snapshots as one base64 attribute.
    if let Some(s) = desc.attribute((NS, "snapshots")) {
        let (e, r) = blob(s, reg)?;
        edit.snapshots = e.snapshots;
        merge(&mut report, r);
    }
    if let Some(snaps) = child(desc, "snapshots") {
        for li in module_items(snaps) {
            let masks = match (child(li, "masks"), li.attribute((NS, "masks"))) {
                (Some(m), _) => masks::read_masks(m, reg, &mut partial),
                (None, Some(s)) => blob(s, reg)?.0.masks,
                (None, None) => Vec::new(),
            };
            let modules = match child(li, "modules") {
                Some(m) => read_modules(m, reg, &mut report, &mut unknown, &mut partial)?,
                None => Vec::new(),
            };
            edit.snapshots.push(crate::model::Snapshot {
                name: li.attribute((NS, "name")).unwrap_or_default().to_owned(),
                modules,
                masks,
            });
        }
    }
    let modules = child(desc, "modules").ok_or_else(|| bad("no module list"))?;
    edit.modules = read_modules(modules, reg, &mut report, &mut unknown, &mut partial)?;
    let history = match child(desc, "history") {
        Some(h) if report.migrated.is_empty() && unknown.is_empty() && !partial => {
            let b = b64::decode(h.text().unwrap_or("").trim()).ok_or_else(|| bad("bad base64"))?;
            codec::decode_history(&b, reg)?
        }
        _ => History::default(),
    };
    unknown.sort();
    unknown.dedup();
    Ok(Loaded {
        newer_process: edit.process > crate::model::edit::PROCESS,
        edit,
        report,
        history,
        name: desc.attribute((NS, "name")).map(str::to_owned),
        writer: desc.attribute((NS, "writer")).map(str::to_owned),
        unknown_modules: unknown,
        partial,
    })
}

fn blob(s: &str, reg: &Registry) -> Result<(Edit, MigrationReport), Error> {
    let b = b64::decode(s).ok_or_else(|| bad("bad base64"))?;
    Ok(codec::decode_edit(&b, reg)?)
}

/// The modules of a `spd:modules` element; those this build does not know are named in
/// `unknown` (and kept in the file when it is written again).
fn read_modules(
    modules: roxmltree::Node,
    reg: &Registry,
    report: &mut MigrationReport,
    unknown: &mut Vec<String>,
    partial: &mut bool,
) -> Result<Vec<ModuleInstance>, Error> {
    let num = |n: roxmltree::Node, a: &str| -> Option<u16> { n.attribute((NS, a))?.parse().ok() };
    let mut out = Vec::new();
    for li in module_items(modules) {
        let name = li
            .attribute((NS, "module"))
            .ok_or_else(|| bad("a module without a name"))?;
        let Some(kind) = module_kind(reg, name) else {
            unknown.push(name.to_owned());
            continue;
        };
        let mut version = num(li, "version").ok_or_else(|| bad("a module without a version"))?;
        let instance = num(li, "instance").unwrap_or(0);
        let defaults = reg.get(kind).map(|s| (s.defaults)()).unwrap_or_default();
        let mut fields = Fields::new();
        if let Some(p) = child(li, "params") {
            for a in p.attributes() {
                if a.namespace() == Some(NS) {
                    let v = parse_value(a.value(), defaults.get(a.name()));
                    fields.0.push((a.name().to_owned(), v));
                }
            }
        }
        let params: Params =
            crate::model::migrate::on_load(reg, kind, instance, &mut version, &mut fields, report);
        // A blend this build cannot read: the module is shown off rather than unmasked.
        let mut enabled = li.attribute((NS, "enabled")) != Some("False");
        let blend = match (child(li, "blend"), li.attribute((NS, "blend"))) {
            (Some(b), _) => match masks::read_blend(b, reg) {
                Ok(b) => Some(b),
                Err(_) => {
                    *partial = true;
                    enabled = false;
                    None
                }
            },
            (None, Some(s)) => blob(s, reg)?
                .0
                .modules
                .into_iter()
                .next()
                .and_then(|m| m.blend),
            (None, None) => None,
        };
        out.push(ModuleInstance {
            kind,
            version,
            instance,
            label: li.attribute((NS, "label")).map(str::to_owned),
            enabled,
            params,
            blend,
        });
    }
    Ok(out)
}

fn merge(into: &mut MigrationReport, r: MigrationReport) {
    into.migrated.extend(r.migrated);
    into.not_rendered.extend(r.not_rendered);
}

/// Writes the sidecar of `file` atomically (a temporary file in the same folder, then a
/// rename), so a reader never sees half of it.
pub fn write(file: &Path, edit: &Edit, history: Option<&History>) -> std::io::Result<PathBuf> {
    write_version(file, 0, None, edit, history)
}

/// The sidecar of version `version` of `file`: the first version's is [`path_for`]'s, the
/// others carry the number (`IMG_1234.CR2.v2.specular.xmp`).
pub fn path_for_version(file: &Path, version: u32) -> PathBuf {
    if version == 0 {
        return path_for(file);
    }
    let mut s = file.as_os_str().to_owned();
    s.push(format!(".v{version}{SUFFIX}"));
    PathBuf::from(s)
}

/// The versions above 0 that have a sidecar beside `file`, in order.
pub fn versions_on_disk(file: &Path) -> Vec<u32> {
    let (Some(dir), Some(name)) = (file.parent(), file.file_name().and_then(|n| n.to_str())) else {
        return Vec::new();
    };
    let Ok(rd) = std::fs::read_dir(if dir.as_os_str().is_empty() {
        Path::new(".")
    } else {
        dir
    }) else {
        return Vec::new();
    };
    let mut v: Vec<u32> = rd
        .flatten()
        .filter_map(|e| {
            let n = e.file_name();
            let n = n.to_str()?;
            let rest = n.strip_prefix(name)?.strip_prefix(".v")?;
            let num = rest.strip_suffix(SUFFIX)?;
            num.parse().ok().filter(|&k| k > 0)
        })
        .collect();
    v.sort_unstable();
    v
}

/// The photos in `dir` with more than one version, and how many each has (the first, which
/// needs no sidecar of its own, and those with a numbered sidecar): one listing of `dir`.
pub fn version_counts_in(dir: &Path) -> Vec<(PathBuf, u32)> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut counts: std::collections::BTreeMap<String, u32> = Default::default();
    for e in rd.flatten() {
        let n = e.file_name();
        let Some(n) = n.to_str() else { continue };
        let Some(rest) = n.strip_suffix(SUFFIX) else {
            continue;
        };
        // `IMG.CR2.v2` → `IMG.CR2`, version 2.
        let Some((image, v)) = rest.rsplit_once(".v") else {
            continue;
        };
        if v.parse::<u32>().is_ok_and(|v| v > 0) && !image.is_empty() {
            *counts.entry(image.to_owned()).or_insert(1) += 1;
        }
    }
    counts
        .into_iter()
        .map(|(image, n)| (dir.join(image), n))
        .collect()
}

/// Removes version `version`'s sidecar (absent is fine). The image is never touched.
pub fn remove_version(file: &Path, version: u32) -> std::io::Result<()> {
    match std::fs::remove_file(path_for_version(file, version)) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// Reads version `version`'s sidecar of `file`, if it has one.
pub fn read_version(file: &Path, version: u32, reg: &Registry) -> Result<Option<Loaded>, Error> {
    match std::fs::read_to_string(path_for_version(file, version)) {
        Ok(t) => from_xmp(&t, reg).map(Some),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// [`write`] for version `version`, with its name.
pub fn write_version(
    file: &Path,
    version: u32,
    name: Option<&str>,
    edit: &Edit,
    history: Option<&History>,
) -> std::io::Result<PathBuf> {
    let path = path_for_version(file, version);
    let existing = match std::fs::read(&path) {
        Ok(b) => Some(String::from_utf8(b).map_err(|_| kept())?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e),
    };
    if let Some(t) = &existing
        && !keepable(t)
    {
        return Err(kept());
    }
    let text = to_xmp_over(existing.as_deref(), edit, history, name);
    write_text(&path, &text)?;
    Ok(path)
}

/// Whether a sidecar's text can be written over without losing anything: it reads, and
/// fully (no part left out). Anything else is never overwritten.
pub fn keepable(text: &str) -> bool {
    from_xmp(text, Registry::builtin()).is_ok_and(|l| !l.partial)
}

/// Why a write was refused (see [`is_kept`]).
const KEPT: &str = "the sidecar holds what this build cannot read; it is kept unchanged";

fn kept() -> std::io::Error {
    std::io::Error::other(KEPT)
}

/// Whether `e` is a write refused to keep a sidecar this build cannot fully read.
pub fn is_kept(e: &std::io::Error) -> bool {
    e.kind() == std::io::ErrorKind::Other && e.to_string() == KEPT
}

/// Writes `text` to `path` atomically.
pub fn write_text(path: &Path, text: &str) -> std::io::Result<()> {
    let path = path.to_path_buf();
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(format!(".tmp{}", std::process::id()));
    let tmp = PathBuf::from(tmp);
    let r = (|| {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(text.as_bytes())?;
        f.sync_all()?;
        drop(f);
        std::fs::rename(&tmp, &path)
    })();
    if r.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    r
}

/// Reads the sidecar of `file`, if it has one.
pub fn read(file: &Path, reg: &Registry) -> Result<Option<Loaded>, Error> {
    read_version(file, 0, reg)
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_compat;
