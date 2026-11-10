//! The buildingSMART BCF schemas' rules, restated for checking what is written (and what is
//! read): for each element under its parent, its children in order with their counts, its
//! attributes and the type of its text. Element names, order and counts, required
//! attributes, GUID (lower case in 3.0), IFC GUID, xs:dateTime, colour, number and
//! field-of-view ranges are checked; free strings are not.
//!
//! The rules are written from the XSDs (`markup.xsd`, `visinfo.xsd`, `version.xsd`,
//! `project.xsd`, `extensions.xsd` of BCF-XML `release_2_1` and `release_3_0`); the oracle
//! test runs `xmllint` against the XSDs themselves when they have been fetched.

use std::collections::HashMap;

use crate::bcf::Version;
use crate::ids::{is_date_time, is_guid};

/// Which file of the archive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Markup,
    VisInfo,
    Version,
    Project,
    Extensions,
}

impl Kind {
    /// The kind of an archive member by its name.
    pub fn of(name: &str) -> Option<Kind> {
        let n = name.rsplit('/').next().unwrap_or(name).to_ascii_lowercase();
        Some(if n == "markup.bcf" {
            Kind::Markup
        } else if n.ends_with(".bcfv") {
            Kind::VisInfo
        } else if n == "bcf.version" {
            Kind::Version
        } else if n == "project.bcfp" {
            Kind::Project
        } else if n == "extensions.xml" {
            Kind::Extensions
        } else {
            return None;
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Ty {
    Str,
    NonEmpty,
    Guid,
    IfcGuid,
    DateTime,
    Int,
    Double,
    Positive,
    Bool,
    /// Exclusive or inclusive range.
    Range(f64, f64, bool),
    Color,
    Bitmap,
    Uri,
}

fn ty(s: &str, v: Version) -> Ty {
    match s {
        "str" => Ty::Str,
        "ne" if v == Version::V3_0 => Ty::NonEmpty,
        "ne" => Ty::Str,
        "guid" => Ty::Guid,
        "ifcguid" => Ty::IfcGuid,
        "dt" => Ty::DateTime,
        "int" => Ty::Int,
        "dbl" => Ty::Double,
        "pos" => Ty::Positive,
        "bool" => Ty::Bool,
        "fov" if v == Version::V3_0 => Ty::Range(0.0, 180.0, false),
        "fov" => Ty::Range(45.0, 60.0, true),
        "color" => Ty::Color,
        "bitmap" => Ty::Bitmap,
        "uri" => Ty::Uri,
        other => panic!("rule type {other}"),
    }
}

/// One child slot: names (alternatives), at least, at most.
#[derive(Clone, Debug)]
struct Slot {
    names: Vec<String>,
    min: usize,
    max: usize,
}

#[derive(Clone, Debug)]
struct Rule {
    /// Children in order; `None`: text of `text`.
    seq: Option<Vec<Slot>>,
    text: Option<Ty>,
    /// (name, required, type).
    attrs: Vec<(String, bool, Ty)>,
}

/// `"Header?,Topic,Comment*,A|B"` → slots.
fn slots(s: &str) -> Vec<Slot> {
    s.split(',')
        .filter(|p| !p.trim().is_empty())
        .map(|p| {
            let p = p.trim();
            let (name, min, max) = match p.chars().last() {
                Some('?') => (&p[..p.len() - 1], 0, 1),
                Some('*') => (&p[..p.len() - 1], 0, usize::MAX),
                Some('+') => (&p[..p.len() - 1], 1, usize::MAX),
                _ => (p, 1, 1),
            };
            Slot {
                names: name.split('|').map(str::to_owned).collect(),
                min,
                max,
            }
        })
        .collect()
}

/// `"Guid!:guid,TopicType:ne"` → attributes.
fn attrs(s: &str, v: Version) -> Vec<(String, bool, Ty)> {
    s.split(',')
        .filter(|p| !p.trim().is_empty())
        .map(|p| {
            let (n, t) = p.split_once(':').unwrap_or((p, "str"));
            let req = n.ends_with('!');
            (n.trim_end_matches('!').to_owned(), req, ty(t, v))
        })
        .collect()
}

struct Rules(HashMap<String, Rule>);

impl Rules {
    fn el(&mut self, key: &str, children: &str, attr: &str, v: Version) {
        self.0.insert(
            key.to_owned(),
            Rule {
                seq: Some(slots(children)),
                text: None,
                attrs: attrs(attr, v),
            },
        );
    }

    fn leaf(&mut self, key: &str, t: &str, attr: &str, v: Version) {
        self.0.insert(
            key.to_owned(),
            Rule {
                seq: None,
                text: Some(ty(t, v)),
                attrs: attrs(attr, v),
            },
        );
    }

    /// Point-like children X, Y, Z under each of `parents`.
    fn points(&mut self, parent: &str, names: &[&str], v: Version) {
        for n in names {
            self.el(&format!("{parent}>{n}"), "X,Y,Z", "", v);
            for c in ["X", "Y", "Z"] {
                self.leaf(&format!("{n}>{c}"), "dbl", "", v);
            }
        }
    }
}

fn rules(kind: Kind, v: Version) -> Rules {
    let v3 = v == Version::V3_0;
    let mut r = Rules(HashMap::new());
    match kind {
        Kind::Version => {
            if v3 {
                r.el(">Version", "", "VersionId!:str", v);
            } else {
                r.el(">Version", "DetailedVersion?", "VersionId:str", v);
                r.leaf("Version>DetailedVersion", "str", "", v);
            }
        }
        Kind::Project => {
            if v3 {
                r.el(">ProjectInfo", "Project", "", v);
                r.el("ProjectInfo>Project", "Name?", "ProjectId!:ne", v);
            } else {
                r.el(">ProjectExtension", "Project?,ExtensionSchema", "", v);
                r.el("ProjectExtension>Project", "Name?", "ProjectId!:str", v);
                r.leaf("ProjectExtension>ExtensionSchema", "uri", "", v);
            }
            r.leaf("Project>Name", "ne", "", v);
        }
        Kind::Extensions => {
            r.el(
                ">Extensions",
                "TopicTypes?,TopicStatuses?,Priorities?,TopicLabels?,Users?,SnippetTypes?,Stages?",
                "",
                v,
            );
            for (w, i) in [
                ("TopicTypes", "TopicType"),
                ("TopicStatuses", "TopicStatus"),
                ("Priorities", "Priority"),
                ("TopicLabels", "TopicLabel"),
                ("Users", "User"),
                ("SnippetTypes", "SnippetType"),
                ("Stages", "Stage"),
            ] {
                r.el(&format!("Extensions>{w}"), &format!("{i}*"), "", v);
                r.leaf(&format!("{w}>{i}"), "ne", "", v);
            }
        }
        Kind::Markup => {
            let ext = if v3 { "IsExternal" } else { "isExternal" };
            if v3 {
                r.el(">Markup", "Header?,Topic", "", v);
                r.el("Markup>Header", "Files?", "", v);
                r.el("Header>Files", "File*", "", v);
                r.el(
                    "Files>File",
                    "Filename?,Date?,Reference?",
                    &format!("IfcProject:ifcguid,IfcSpatialStructureElement:ifcguid,{ext}:bool"),
                    v,
                );
                r.el(
                    "Markup>Topic",
                    "ReferenceLinks?,Title,Priority?,Index?,Labels?,CreationDate,CreationAuthor,\
                     ModifiedDate?,ModifiedAuthor?,DueDate?,AssignedTo?,Stage?,Description?,\
                     BimSnippet?,DocumentReferences?,RelatedTopics?,Comments?,Viewpoints?",
                    "Guid!:guid,ServerAssignedId:ne,TopicType!:ne,TopicStatus!:ne",
                    v,
                );
                r.el("Topic>ReferenceLinks", "ReferenceLink*", "", v);
                r.leaf("ReferenceLinks>ReferenceLink", "ne", "", v);
                r.el("Topic>Labels", "Label*", "", v);
                r.leaf("Labels>Label", "ne", "", v);
                r.el("Topic>DocumentReferences", "DocumentReference*", "", v);
                r.el(
                    "DocumentReferences>DocumentReference",
                    "DocumentGuid|Url?,Description?",
                    "Guid!:guid",
                    v,
                );
                r.leaf("DocumentReference>DocumentGuid", "guid", "", v);
                r.leaf("DocumentReference>Url", "ne", "", v);
                r.el("Topic>RelatedTopics", "RelatedTopic*", "", v);
                r.el("RelatedTopics>RelatedTopic", "", "Guid!:guid", v);
                r.el("Topic>Comments", "Comment*", "", v);
                r.el(
                    "Comments>Comment",
                    "Date,Author,Comment?,Viewpoint?,ModifiedDate?,ModifiedAuthor?",
                    "Guid!:guid",
                    v,
                );
                r.el("Topic>Viewpoints", "ViewPoint*", "", v);
                r.el(
                    "Viewpoints>ViewPoint",
                    "Viewpoint?,Snapshot?,Index?",
                    "Guid!:guid",
                    v,
                );
                r.leaf("ViewPoint>Viewpoint", "ne", "", v);
                r.leaf("ViewPoint>Snapshot", "ne", "", v);
                r.leaf("ViewPoint>Index", "int", "", v);
            } else {
                r.el(">Markup", "Header?,Topic,Comment*,Viewpoints*", "", v);
                r.el("Markup>Header", "File+", "", v);
                r.el(
                    "Header>File",
                    "Filename?,Date?,Reference?",
                    &format!("IfcProject:ifcguid,IfcSpatialStructureElement:ifcguid,{ext}:bool"),
                    v,
                );
                r.el(
                    "Markup>Topic",
                    "ReferenceLink*,Title,Priority?,Index?,Labels*,CreationDate,CreationAuthor,\
                     ModifiedDate?,ModifiedAuthor?,DueDate?,AssignedTo?,Stage?,Description?,\
                     BimSnippet?,DocumentReference*,RelatedTopic*",
                    "Guid!:guid,TopicType:str,TopicStatus:str",
                    v,
                );
                r.leaf("Topic>ReferenceLink", "str", "", v);
                r.leaf("Topic>Labels", "str", "", v);
                r.el(
                    "Topic>DocumentReference",
                    "ReferencedDocument?,Description?",
                    "Guid:guid,isExternal:bool",
                    v,
                );
                r.leaf("DocumentReference>ReferencedDocument", "str", "", v);
                r.el("Topic>RelatedTopic", "", "Guid!:guid", v);
                r.el(
                    "Markup>Comment",
                    "Date,Author,Comment,Viewpoint?,ModifiedDate?,ModifiedAuthor?",
                    "Guid!:guid",
                    v,
                );
                r.el(
                    "Markup>Viewpoints",
                    "Viewpoint?,Snapshot?,Index?",
                    "Guid!:guid",
                    v,
                );
                r.leaf("Viewpoints>Viewpoint", "str", "", v);
                r.leaf("Viewpoints>Snapshot", "str", "", v);
                r.leaf("Viewpoints>Index", "int", "", v);
            }
            r.leaf("File>Filename", "ne", "", v);
            r.leaf("File>Date", "dt", "", v);
            r.leaf("File>Reference", "ne", "", v);
            r.leaf("Topic>Title", "ne", "", v);
            r.leaf("Topic>Priority", "ne", "", v);
            r.leaf("Topic>Index", "int", "", v);
            r.leaf("Topic>CreationDate", "dt", "", v);
            r.leaf("Topic>CreationAuthor", "ne", "", v);
            r.leaf("Topic>ModifiedDate", "dt", "", v);
            r.leaf("Topic>ModifiedAuthor", "ne", "", v);
            r.leaf("Topic>DueDate", "dt", "", v);
            r.leaf("Topic>AssignedTo", "ne", "", v);
            r.leaf("Topic>Stage", "ne", "", v);
            r.leaf("Topic>Description", "ne", "", v);
            r.el(
                "Topic>BimSnippet",
                "Reference,ReferenceSchema",
                &format!("SnippetType!:ne,{ext}:bool"),
                v,
            );
            r.leaf("BimSnippet>Reference", "ne", "", v);
            r.leaf("BimSnippet>ReferenceSchema", "ne", "", v);
            r.leaf("DocumentReference>Description", "ne", "", v);
            r.leaf("Comment>Date", "dt", "", v);
            r.leaf("Comment>Author", "ne", "", v);
            r.leaf("Comment>Comment", "ne", "", v);
            r.el("Comment>Viewpoint", "", "Guid!:guid", v);
            r.leaf("Comment>ModifiedDate", "dt", "", v);
            r.leaf("Comment>ModifiedAuthor", "ne", "", v);
        }
        Kind::VisInfo => {
            if v3 {
                r.el(
                    ">VisualizationInfo",
                    "Components?,OrthogonalCamera|PerspectiveCamera,Lines?,ClippingPlanes?,Bitmaps?",
                    "Guid!:guid",
                    v,
                );
                r.el(
                    "VisualizationInfo>Components",
                    "Selection?,Visibility?,Coloring?",
                    "",
                    v,
                );
                r.el("Components>Selection", "Component*", "", v);
                r.el(
                    "Components>Visibility",
                    "ViewSetupHints?,Exceptions?",
                    "DefaultVisibility:bool",
                    v,
                );
                r.el("Visibility>Exceptions", "Component*", "", v);
                r.el("Coloring>Color", "Components", "Color!:color", v);
                r.el("Color>Components", "Component+", "", v);
                r.el("Components>Coloring", "Color*", "", v);
                r.el("VisualizationInfo>Lines", "Line*", "", v);
                r.el("VisualizationInfo>ClippingPlanes", "ClippingPlane*", "", v);
                r.el("VisualizationInfo>Bitmaps", "Bitmap*", "", v);
                r.el(
                    "Bitmaps>Bitmap",
                    "Format,Reference,Location,Normal,Up,Height",
                    "",
                    v,
                );
                r.leaf("Bitmap>Format", "bitmap", "", v);
                r.points("Bitmap", &["Location", "Normal", "Up"], v);
                for c in ["OrthogonalCamera", "PerspectiveCamera"] {
                    let field = if c == "OrthogonalCamera" {
                        "ViewToWorldScale"
                    } else {
                        "FieldOfView"
                    };
                    r.el(
                        &format!("VisualizationInfo>{c}"),
                        &format!(
                            "CameraViewPoint,CameraDirection,CameraUpVector,{field},AspectRatio"
                        ),
                        "",
                        v,
                    );
                    r.leaf(&format!("{c}>AspectRatio"), "pos", "", v);
                }
            } else {
                r.el(
                    ">VisualizationInfo",
                    "Components?,OrthogonalCamera?,PerspectiveCamera?,Lines?,ClippingPlanes?,Bitmap*",
                    "Guid!:guid",
                    v,
                );
                r.el(
                    "VisualizationInfo>Components",
                    "ViewSetupHints?,Selection?,Visibility,Coloring?",
                    "",
                    v,
                );
                r.el("Components>Selection", "Component+", "", v);
                r.el(
                    "Components>Visibility",
                    "Exceptions?",
                    "DefaultVisibility:bool",
                    v,
                );
                r.el("Visibility>Exceptions", "Component+", "", v);
                r.el("Components>Coloring", "Color+", "", v);
                r.el("Coloring>Color", "Component+", "Color!:color", v);
                r.el("VisualizationInfo>Lines", "Line+", "", v);
                r.el("VisualizationInfo>ClippingPlanes", "ClippingPlane*", "", v);
                r.el(
                    "VisualizationInfo>Bitmap",
                    "Bitmap,Reference,Location,Normal,Up,Height",
                    "",
                    v,
                );
                r.leaf("Bitmap>Bitmap", "bitmap", "", v);
                r.points("Bitmap", &["Location", "Normal", "Up"], v);
                for c in ["OrthogonalCamera", "PerspectiveCamera"] {
                    let field = if c == "OrthogonalCamera" {
                        "ViewToWorldScale"
                    } else {
                        "FieldOfView"
                    };
                    r.el(
                        &format!("VisualizationInfo>{c}"),
                        &format!("CameraViewPoint,CameraDirection,CameraUpVector,{field}"),
                        "",
                        v,
                    );
                }
            }
            r.leaf("Bitmap>Reference", "ne", "", v);
            r.leaf("Bitmap>Height", "dbl", "", v);
            let hints = if v3 { "Visibility" } else { "Components" };
            r.el(
                &format!("{hints}>ViewSetupHints"),
                "",
                "SpacesVisible:bool,SpaceBoundariesVisible:bool,OpeningsVisible:bool",
                v,
            );
            for p in ["Selection", "Exceptions", "Color", "Components"] {
                r.el(
                    &format!("{p}>Component"),
                    "OriginatingSystem?,AuthoringToolId?",
                    "IfcGuid:ifcguid",
                    v,
                );
            }
            r.leaf("Component>OriginatingSystem", "ne", "", v);
            r.leaf("Component>AuthoringToolId", "ne", "", v);
            for c in ["OrthogonalCamera", "PerspectiveCamera"] {
                r.points(
                    c,
                    &["CameraViewPoint", "CameraDirection", "CameraUpVector"],
                    v,
                );
            }
            r.leaf("OrthogonalCamera>ViewToWorldScale", "dbl", "", v);
            r.leaf("PerspectiveCamera>FieldOfView", "fov", "", v);
            r.el("Lines>Line", "StartPoint,EndPoint", "", v);
            r.points("Line", &["StartPoint", "EndPoint"], v);
            r.el("ClippingPlanes>ClippingPlane", "Location,Direction", "", v);
            r.points("ClippingPlane", &["Location", "Direction"], v);
        }
    }
    r
}

fn ok_text(t: Ty, s: &str, v: Version) -> bool {
    match t {
        Ty::Str | Ty::Uri => true,
        Ty::NonEmpty => !s.is_empty(),
        Ty::Guid => is_guid(s.trim(), v == Version::V3_0),
        Ty::IfcGuid => s.chars().all(|c| {
            c.is_ascii_alphanumeric() || c == '_' || c == '$' || (c == ',' && v == Version::V2_1)
        }),
        Ty::DateTime => is_date_time(s.trim()),
        Ty::Int => s.trim().parse::<i32>().is_ok(),
        Ty::Double => s.trim().parse::<f64>().is_ok(),
        Ty::Positive => s.trim().parse::<f64>().is_ok_and(|x| x > 0.0),
        Ty::Bool => matches!(s.trim(), "true" | "false" | "1" | "0"),
        Ty::Range(lo, hi, inclusive) => s.trim().parse::<f64>().is_ok_and(|x| {
            if inclusive {
                (lo..=hi).contains(&x)
            } else {
                x > lo && x < hi
            }
        }),
        Ty::Color => {
            let s = s.trim();
            (s.len() == 6 || s.len() == 8)
                && s.chars().all(|c| {
                    c.is_ascii_digit()
                        || (v == Version::V3_0 && c.is_ascii_hexdigit())
                        || ('A'..='F').contains(&c)
                        || (v == Version::V2_1 && c == ',')
                })
        }
        Ty::Bitmap => match v {
            Version::V3_0 => matches!(s, "png" | "jpg"),
            Version::V2_1 => matches!(s, "PNG" | "JPG"),
        },
    }
}

/// What in `xml` (a member of `kind`) breaks the schema of `v`; empty when it is valid.
pub fn check(xml: &str, kind: Kind, v: Version) -> Vec<String> {
    let doc = match roxmltree::Document::parse(xml.trim_start_matches('\u{feff}')) {
        Ok(d) => d,
        Err(e) => return vec![format!("not XML: {e}")],
    };
    let rules = rules(kind, v);
    let mut errs = Vec::new();
    let root = doc.root_element();
    walk(root, "", &rules, v, &mut errs);
    errs
}

fn walk(n: roxmltree::Node, parent: &str, rules: &Rules, v: Version, errs: &mut Vec<String>) {
    let name = n.tag_name().name();
    let key = format!("{parent}>{name}");
    let Some(rule) = rules.0.get(&key) else {
        errs.push(format!("unexpected element {key}"));
        return;
    };
    for a in n.attributes() {
        if a.namespace().is_some() || a.name().starts_with("xmlns") {
            continue;
        }
        match rule.attrs.iter().find(|(k, _, _)| k == a.name()) {
            None => errs.push(format!("{key}: unexpected attribute {}", a.name())),
            Some((_, _, t)) if !ok_text(*t, a.value(), v) => {
                errs.push(format!("{key}: bad {} = {:?}", a.name(), a.value()));
            }
            _ => {}
        }
    }
    for (k, req, _) in &rule.attrs {
        if *req && n.attribute(k.as_str()).is_none() {
            errs.push(format!("{key}: missing attribute {k}"));
        }
    }
    let kids: Vec<roxmltree::Node> = n.children().filter(|c| c.is_element()).collect();
    match (&rule.seq, rule.text) {
        (None, Some(t)) => {
            if !kids.is_empty() {
                errs.push(format!("{key}: elements in a text element"));
            }
            let s = n.text().unwrap_or("");
            if !ok_text(t, s, v) {
                errs.push(format!("{key}: bad text {s:?}"));
            }
        }
        (Some(seq), _) => {
            let mut i = 0;
            for slot in seq {
                let mut count = 0;
                while i < kids.len()
                    && count < slot.max
                    && slot.names.iter().any(|s| s == kids[i].tag_name().name())
                {
                    count += 1;
                    i += 1;
                }
                if count < slot.min {
                    errs.push(format!("{key}: missing {}", slot.names.join(" or ")));
                }
            }
            if i < kids.len() {
                errs.push(format!("{key}: {} out of place", kids[i].tag_name().name()));
            }
            for k in &kids {
                walk(*k, name, rules, v, errs);
            }
        }
        (None, None) => {}
    }
}

/// Checks every XML member of an archive written as `v`: (member, problem) pairs.
pub fn check_members(members: &[(String, Vec<u8>)], v: Version) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (name, bytes) in members {
        let Some(kind) = Kind::of(name) else { continue };
        let xml = String::from_utf8_lossy(bytes);
        for e in check(&xml, kind, v) {
            out.push((name.clone(), e));
        }
    }
    if v == Version::V3_0 && !members.iter().any(|(n, _)| n == "extensions.xml") {
        out.push(("extensions.xml".into(), "missing".into()));
    }
    if !members.iter().any(|(n, _)| n == "bcf.version") {
        out.push(("bcf.version".into(), "missing".into()));
    }
    out
}
