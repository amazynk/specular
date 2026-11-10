//! The BCF model and its reader: a `.bcfzip` (or `.bcf`) of `bcf.version`, optional project
//! and extension files, and a folder per topic holding `markup.bcf`, its viewpoints
//! (`.bcfv`) and snapshots. Versions 2.0 and 2.1 (comments and viewpoints beside the topic)
//! and 3.0 (inside it, wrapped in lists) read into the same model.
//!
//! Dates stay the strings the file gave, so a topic read and written back keeps them; other
//! files in a topic's folder (bitmaps, documents) are kept as bytes and written back.

use std::io::{Cursor, Read};

/// Members read out of an archive at most, and bytes per member.
const MAX_MEMBERS: usize = 20_000;
const MAX_MEMBER_BYTES: u64 = 256 << 20;

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum Version {
    #[default]
    V2_1,
    V3_0,
}

impl Version {
    pub fn id(self) -> &'static str {
        match self {
            Version::V2_1 => "2.1",
            Version::V3_0 => "3.0",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Archive {
    /// The version read (what is written back unless asked otherwise).
    pub version: Version,
    /// `VersionId` as the file gave it (`2.0`, `2.1`, `3.0`).
    pub version_id: String,
    pub project: Option<Project>,
    pub extensions: Extensions,
    pub topics: Vec<Topic>,
    /// What was not understood and left out, for the user (`"bitmaps in 2 viewpoints"`).
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Project {
    pub id: String,
    pub name: Option<String>,
}

/// BCF 3.0's `extensions.xml`: the values the project allows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Extensions {
    pub topic_types: Vec<String>,
    pub topic_statuses: Vec<String>,
    pub priorities: Vec<String>,
    pub topic_labels: Vec<String>,
    pub users: Vec<String>,
    pub snippet_types: Vec<String>,
    pub stages: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Topic {
    pub guid: String,
    pub topic_type: Option<String>,
    pub status: Option<String>,
    pub server_id: Option<String>,
    pub title: String,
    pub priority: Option<String>,
    pub index: Option<i32>,
    pub labels: Vec<String>,
    pub creation_date: String,
    pub creation_author: String,
    pub modified_date: Option<String>,
    pub modified_author: Option<String>,
    pub due_date: Option<String>,
    pub assigned_to: Option<String>,
    pub stage: Option<String>,
    pub description: Option<String>,
    pub reference_links: Vec<String>,
    pub bim_snippet: Option<BimSnippet>,
    pub documents: Vec<DocumentReference>,
    pub related: Vec<String>,
    /// The model files the topic is about (the markup's header).
    pub files: Vec<HeaderFile>,
    pub comments: Vec<Comment>,
    pub viewpoints: Vec<Viewpoint>,
    /// Other files of the topic's folder (bitmaps, documents): name and bytes.
    pub extra_files: Vec<(String, Vec<u8>)>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BimSnippet {
    pub snippet_type: String,
    pub is_external: bool,
    pub reference: String,
    pub reference_schema: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DocumentReference {
    pub guid: Option<String>,
    pub is_external: bool,
    /// The document (2.x `ReferencedDocument`, 3.0 `Url`), or 3.0's `DocumentGuid`.
    pub referenced: Option<String>,
    pub document_guid: Option<String>,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct HeaderFile {
    pub ifc_project: Option<String>,
    pub ifc_spatial: Option<String>,
    pub is_external: bool,
    pub filename: Option<String>,
    pub date: Option<String>,
    pub reference: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Comment {
    pub guid: String,
    pub date: String,
    pub author: String,
    pub text: String,
    /// The viewpoint it refers to (GUID).
    pub viewpoint: Option<String>,
    pub modified_date: Option<String>,
    pub modified_author: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Viewpoint {
    pub guid: String,
    /// The `.bcfv` file's name in the topic's folder.
    pub file: Option<String>,
    pub snapshot_name: Option<String>,
    pub snapshot: Option<Vec<u8>>,
    pub index: Option<i32>,
    pub vis: Option<VisInfo>,
}

/// A `.bcfv` file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VisInfo {
    pub guid: String,
    pub components: Option<Components>,
    pub camera: Option<BcfCamera>,
    pub lines: Vec<([f64; 3], [f64; 3])>,
    pub clipping_planes: Vec<ClippingPlane>,
    pub bitmaps: Vec<Bitmap>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Components {
    pub selection: Vec<Component>,
    pub default_visibility: bool,
    pub exceptions: Vec<Component>,
    pub hints: Option<ViewSetupHints>,
    pub coloring: Vec<Coloring>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ViewSetupHints {
    pub spaces_visible: Option<bool>,
    pub space_boundaries_visible: Option<bool>,
    pub openings_visible: Option<bool>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Coloring {
    /// `RRGGBB` or `AARRGGBB` hex, as given.
    pub color: String,
    pub components: Vec<Component>,
}

impl Coloring {
    /// The colour as straight sRGB RGBA (BCF's eight digits put alpha first).
    pub fn rgba(&self) -> Option<[u8; 4]> {
        let h = self.color.trim();
        let v = |i: usize| u8::from_str_radix(h.get(i..i + 2)?, 16).ok();
        match h.len() {
            6 => Some([v(0)?, v(2)?, v(4)?, 255]),
            8 => Some([v(2)?, v(4)?, v(6)?, v(0)?]),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Component {
    pub ifc_guid: Option<String>,
    pub originating_system: Option<String>,
    pub authoring_tool_id: Option<String>,
}

impl Component {
    pub fn of(guid: &str) -> Component {
        Component {
            ifc_guid: Some(guid.to_owned()),
            ..Component::default()
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CameraKind {
    /// Vertical field of view in degrees (the common reading of BCF's `FieldOfView`).
    Perspective { fov: f64 },
    /// The view's height in metres (`ViewToWorldScale`).
    Orthogonal { scale: f64 },
}

/// A viewpoint's camera, in metres in the project's frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BcfCamera {
    pub kind: CameraKind,
    pub viewpoint: [f64; 3],
    pub direction: [f64; 3],
    pub up: [f64; 3],
    /// Width over height (3.0).
    pub aspect: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClippingPlane {
    pub location: [f64; 3],
    /// Points to the side that is cut away.
    pub direction: [f64; 3],
}

#[derive(Clone, Debug, PartialEq)]
pub struct Bitmap {
    /// `png` or `jpg`.
    pub format: String,
    pub reference: String,
    pub location: [f64; 3],
    pub normal: [f64; 3],
    pub up: [f64; 3],
    pub height: f64,
}

/// Why an archive could not be read.
#[derive(Clone, Debug, PartialEq)]
pub enum BcfError {
    NotZip(String),
    Malformed(String),
    TooLarge,
}

impl std::fmt::Display for BcfError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BcfError::NotZip(m) => write!(f, "not a BCF archive: {m}"),
            BcfError::Malformed(m) => write!(f, "malformed BCF: {m}"),
            BcfError::TooLarge => write!(f, "the BCF archive is too large"),
        }
    }
}

impl std::error::Error for BcfError {}

type Node<'a, 'i> = roxmltree::Node<'a, 'i>;

fn child<'a, 'i>(n: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    n.children()
        .find(|c| c.is_element() && c.tag_name().name() == name)
}

fn children<'a, 'i>(n: Node<'a, 'i>, name: &'a str) -> impl Iterator<Item = Node<'a, 'i>> + 'a {
    n.children()
        .filter(move |c| c.is_element() && c.tag_name().name() == name)
}

fn text(n: Node, name: &str) -> Option<String> {
    child(n, name).map(|c| c.text().unwrap_or("").to_owned())
}

fn attr(n: Node, name: &str) -> Option<String> {
    n.attributes()
        .find(|a| a.name().eq_ignore_ascii_case(name))
        .map(|a| a.value().to_owned())
}

fn bool_attr(n: Node, name: &str) -> Option<bool> {
    attr(n, name).map(|v| matches!(v.trim(), "true" | "1"))
}

fn num(n: Node, name: &str) -> Option<f64> {
    text(n, name).and_then(|t| t.trim().parse().ok())
}

fn point(n: Node) -> Option<[f64; 3]> {
    Some([num(n, "X")?, num(n, "Y")?, num(n, "Z")?])
}

fn point_in(n: Node, name: &str) -> Option<[f64; 3]> {
    child(n, name).and_then(point)
}

fn parse(xml: &str) -> Result<roxmltree::Document<'_>, BcfError> {
    roxmltree::Document::parse_with_options(
        xml,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: 50_000_000,
            ..roxmltree::ParsingOptions::default()
        },
    )
    .map_err(|e| BcfError::Malformed(e.to_string()))
}

/// The text of a member, without a byte-order mark.
fn utf8(b: &[u8]) -> Result<&str, BcfError> {
    let b = b.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(b);
    std::str::from_utf8(b).map_err(|_| BcfError::Malformed("a file is not UTF-8".into()))
}

/// Reads a `.bcfzip` / `.bcf` archive.
pub fn read(bytes: &[u8]) -> Result<Archive, BcfError> {
    let mut zip =
        zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| BcfError::NotZip(e.to_string()))?;
    if zip.len() > MAX_MEMBERS {
        return Err(BcfError::TooLarge);
    }
    let mut members: Vec<(String, Vec<u8>)> = Vec::with_capacity(zip.len());
    for i in 0..zip.len() {
        let mut f = zip
            .by_index(i)
            .map_err(|e| BcfError::Malformed(e.to_string()))?;
        if f.is_dir() {
            continue;
        }
        if f.size() > MAX_MEMBER_BYTES {
            return Err(BcfError::TooLarge);
        }
        let name = f
            .name()
            .map_err(|e| BcfError::Malformed(e.to_string()))?
            .replace('\\', "/");
        let mut v = Vec::with_capacity(f.size() as usize);
        (&mut f)
            .take(MAX_MEMBER_BYTES)
            .read_to_end(&mut v)
            .map_err(|e| BcfError::Malformed(e.to_string()))?;
        members.push((name, v));
    }
    // A root folder around everything (some tools zip the folder): strip it.
    let prefix = common_root(&members);
    let get = |name: &str| {
        members
            .iter()
            .find(|(n, _)| n[prefix.len()..].eq_ignore_ascii_case(name))
            .map(|(_, b)| b.as_slice())
    };
    let mut a = Archive::default();
    match get("bcf.version") {
        Some(v) => {
            let s = utf8(v)?;
            let d = parse(s)?;
            a.version_id = attr(d.root_element(), "VersionId").unwrap_or_default();
        }
        None => {
            a.version_id = "2.1".into();
            a.notes.push("no bcf.version: read as 2.1".into());
        }
    }
    a.version = if a.version_id.trim().starts_with('3') {
        Version::V3_0
    } else {
        Version::V2_1
    };
    if let Some(p) = get("project.bcfp") {
        let d = parse(utf8(p)?)?;
        let root = d.root_element();
        if let Some(pr) = child(root, "Project") {
            a.project = Some(Project {
                id: attr(pr, "ProjectId").unwrap_or_default(),
                name: text(pr, "Name"),
            });
        }
    }
    if let Some(e) = get("extensions.xml") {
        let d = parse(utf8(e)?)?;
        a.extensions = read_extensions(d.root_element());
    }
    // Topics: every folder with a markup.bcf.
    let mut folders: Vec<String> = members
        .iter()
        .filter_map(|(n, _)| {
            let rel = &n[prefix.len()..];
            let (dir, file) = rel.rsplit_once('/')?;
            (file.eq_ignore_ascii_case("markup.bcf") && !dir.contains('/')).then(|| dir.to_owned())
        })
        .collect();
    folders.sort();
    for dir in folders {
        let in_dir: Vec<(&str, &[u8])> = members
            .iter()
            .filter_map(|(n, b)| {
                let rel = &n[prefix.len()..];
                let (d, f) = rel.split_once('/')?;
                (d == dir && !f.contains('/')).then_some((f, b.as_slice()))
            })
            .collect();
        let markup = in_dir
            .iter()
            .find(|(f, _)| f.eq_ignore_ascii_case("markup.bcf"))
            .map(|(_, b)| *b)
            .unwrap_or_default();
        let mut t = read_markup(utf8(markup)?, a.version)?;
        if t.guid.is_empty() {
            t.guid = dir.clone();
        }
        let mut used: Vec<String> = vec!["markup.bcf".into()];
        for vp in &mut t.viewpoints {
            if let Some(f) = vp.file.clone()
                && let Some((name, b)) = in_dir.iter().find(|(n, _)| n.eq_ignore_ascii_case(&f))
            {
                let mut vis = read_visinfo(utf8(b)?, a.version)?;
                if vis.guid.trim().is_empty() {
                    vis.guid = vp.guid.clone();
                }
                vp.vis = Some(vis);
                used.push((*name).to_owned());
            }
            if let Some(s) = vp.snapshot_name.clone()
                && let Some((name, b)) = in_dir.iter().find(|(n, _)| n.eq_ignore_ascii_case(&s))
            {
                vp.snapshot = Some(b.to_vec());
                used.push((*name).to_owned());
            }
        }
        for (f, b) in &in_dir {
            if !used.iter().any(|u| u.eq_ignore_ascii_case(f)) {
                t.extra_files.push(((*f).to_owned(), b.to_vec()));
            }
        }
        a.topics.push(t);
    }
    a.topics.sort_by(|x, y| {
        x.index
            .unwrap_or(i32::MAX)
            .cmp(&y.index.unwrap_or(i32::MAX))
            .then_with(|| x.creation_date.cmp(&y.creation_date))
    });
    Ok(a)
}

/// The folder every member is in, with its slash (`""` for none).
fn common_root(members: &[(String, Vec<u8>)]) -> String {
    if members
        .iter()
        .any(|(n, _)| n.eq_ignore_ascii_case("bcf.version"))
    {
        return String::new();
    }
    let Some((first, _)) = members.first() else {
        return String::new();
    };
    let Some((root, _)) = first.split_once('/') else {
        return String::new();
    };
    let p = format!("{root}/");
    if members.iter().all(|(n, _)| {
        n.starts_with(&p) && n[p.len()..].contains('/')
            || n.eq_ignore_ascii_case(&format!("{p}bcf.version"))
    }) && members
        .iter()
        .any(|(n, _)| n.eq_ignore_ascii_case(&format!("{p}bcf.version")))
    {
        p
    } else {
        String::new()
    }
}

fn read_extensions(root: Node) -> Extensions {
    let list = |wrap: &str, item: &str| -> Vec<String> {
        child(root, wrap)
            .map(|w| {
                w.children()
                    .filter(|c| c.is_element() && c.tag_name().name() == item)
                    .map(|c| c.text().unwrap_or("").to_owned())
                    .collect()
            })
            .unwrap_or_default()
    };
    Extensions {
        topic_types: list("TopicTypes", "TopicType"),
        topic_statuses: list("TopicStatuses", "TopicStatus"),
        priorities: list("Priorities", "Priority"),
        topic_labels: list("TopicLabels", "TopicLabel"),
        users: list("Users", "User"),
        snippet_types: list("SnippetTypes", "SnippetType"),
        stages: list("Stages", "Stage"),
    }
}

/// Items of `name` directly under `n` and under its `wrap` list (3.0 wraps, 2.x does not).
fn items<'a, 'i>(n: Node<'a, 'i>, wrap: &str, name: &'a str) -> Vec<Node<'a, 'i>> {
    let mut v: Vec<Node> = children(n, name).collect();
    if let Some(w) = child(n, wrap) {
        v.extend(children(w, name));
    }
    v
}

fn read_markup(xml: &str, version: Version) -> Result<Topic, BcfError> {
    let d = parse(xml)?;
    let root = d.root_element();
    let tn =
        child(root, "Topic").ok_or_else(|| BcfError::Malformed("a markup has no topic".into()))?;
    let mut t = Topic {
        guid: attr(tn, "Guid").unwrap_or_default(),
        topic_type: attr(tn, "TopicType"),
        status: attr(tn, "TopicStatus"),
        server_id: attr(tn, "ServerAssignedId"),
        title: text(tn, "Title").unwrap_or_default(),
        priority: text(tn, "Priority"),
        index: text(tn, "Index").and_then(|s| s.trim().parse().ok()),
        creation_date: text(tn, "CreationDate").unwrap_or_default(),
        creation_author: text(tn, "CreationAuthor").unwrap_or_default(),
        modified_date: text(tn, "ModifiedDate"),
        modified_author: text(tn, "ModifiedAuthor"),
        due_date: text(tn, "DueDate"),
        assigned_to: text(tn, "AssignedTo"),
        stage: text(tn, "Stage"),
        description: text(tn, "Description"),
        ..Topic::default()
    };
    // 2.x repeats Labels; 3.0 wraps Label items in Labels.
    for l in children(tn, "Labels") {
        let inner: Vec<Node> = children(l, "Label").collect();
        if inner.is_empty() {
            if let Some(s) = l.text().filter(|s| !s.trim().is_empty()) {
                t.labels.push(s.to_owned());
            }
        } else {
            t.labels
                .extend(inner.iter().map(|c| c.text().unwrap_or("").to_owned()));
        }
    }
    t.reference_links = items(tn, "ReferenceLinks", "ReferenceLink")
        .into_iter()
        .map(|c| c.text().unwrap_or("").to_owned())
        .collect();
    if let Some(s) = child(tn, "BimSnippet") {
        t.bim_snippet = Some(BimSnippet {
            snippet_type: attr(s, "SnippetType").unwrap_or_default(),
            is_external: bool_attr(s, "isExternal").unwrap_or(false),
            reference: text(s, "Reference").unwrap_or_default(),
            reference_schema: text(s, "ReferenceSchema").unwrap_or_default(),
        });
    }
    for r in items(tn, "DocumentReferences", "DocumentReference") {
        t.documents.push(DocumentReference {
            guid: attr(r, "Guid"),
            is_external: bool_attr(r, "isExternal").unwrap_or(false),
            referenced: text(r, "ReferencedDocument").or_else(|| text(r, "Url")),
            document_guid: text(r, "DocumentGuid"),
            description: text(r, "Description"),
        });
    }
    t.related = items(tn, "RelatedTopics", "RelatedTopic")
        .into_iter()
        .filter_map(|r| attr(r, "Guid"))
        .collect();
    if let Some(h) = child(root, "Header") {
        for f in items(h, "Files", "File") {
            t.files.push(HeaderFile {
                ifc_project: attr(f, "IfcProject"),
                ifc_spatial: attr(f, "IfcSpatialStructureElement"),
                is_external: bool_attr(f, "isExternal").unwrap_or(true),
                filename: text(f, "Filename"),
                date: text(f, "Date"),
                reference: text(f, "Reference"),
            });
        }
    }
    // Comments and viewpoints: beside the topic in 2.x, inside it in 3.0 (either is read).
    let mut comments: Vec<Node> = children(root, "Comment").collect();
    comments.extend(items(tn, "Comments", "Comment"));
    for c in comments {
        t.comments.push(Comment {
            guid: attr(c, "Guid").unwrap_or_default(),
            date: text(c, "Date").unwrap_or_default(),
            author: text(c, "Author").unwrap_or_default(),
            text: text(c, "Comment").unwrap_or_default(),
            viewpoint: child(c, "Viewpoint").and_then(|v| attr(v, "Guid")),
            modified_date: text(c, "ModifiedDate"),
            modified_author: text(c, "ModifiedAuthor"),
        });
    }
    let mut vps: Vec<Node> = children(root, "Viewpoints").collect();
    if let Some(w) = child(tn, "Viewpoints") {
        vps.extend(children(w, "ViewPoint"));
    }
    for v in vps {
        t.viewpoints.push(Viewpoint {
            guid: attr(v, "Guid").unwrap_or_default(),
            file: text(v, "Viewpoint").filter(|s| !s.trim().is_empty()),
            snapshot_name: text(v, "Snapshot").filter(|s| !s.trim().is_empty()),
            snapshot: None,
            index: text(v, "Index").and_then(|s| s.trim().parse().ok()),
            vis: None,
        });
    }
    let _ = version;
    Ok(t)
}

fn components(n: Node) -> Vec<Component> {
    children(n, "Component")
        .map(|c| Component {
            ifc_guid: attr(c, "IfcGuid"),
            originating_system: text(c, "OriginatingSystem"),
            authoring_tool_id: text(c, "AuthoringToolId"),
        })
        .collect()
}

fn hints(n: Node) -> ViewSetupHints {
    ViewSetupHints {
        spaces_visible: bool_attr(n, "SpacesVisible"),
        space_boundaries_visible: bool_attr(n, "SpaceBoundariesVisible"),
        openings_visible: bool_attr(n, "OpeningsVisible"),
    }
}

/// Reads a `.bcfv` file.
pub fn read_visinfo(xml: &str, _version: Version) -> Result<VisInfo, BcfError> {
    let d = parse(xml)?;
    let root = d.root_element();
    let mut v = VisInfo {
        guid: attr(root, "Guid").unwrap_or_default(),
        ..VisInfo::default()
    };
    if let Some(c) = child(root, "Components") {
        let mut out = Components::default();
        if let Some(h) = child(c, "ViewSetupHints") {
            out.hints = Some(hints(h));
        }
        if let Some(s) = child(c, "Selection") {
            out.selection = components(s);
        }
        if let Some(vis) = child(c, "Visibility") {
            // 2.x has no default; 3.0 defaults to false.
            out.default_visibility = bool_attr(vis, "DefaultVisibility").unwrap_or(false);
            if let Some(h) = child(vis, "ViewSetupHints") {
                out.hints = Some(hints(h));
            }
            if let Some(e) = child(vis, "Exceptions") {
                out.exceptions = components(e);
            }
        } else {
            out.default_visibility = true;
        }
        if let Some(col) = child(c, "Coloring") {
            for k in children(col, "Color") {
                let comps = match child(k, "Components") {
                    Some(w) => components(w),
                    None => components(k),
                };
                out.coloring.push(Coloring {
                    color: attr(k, "Color").unwrap_or_default().to_ascii_uppercase(),
                    components: comps,
                });
            }
        }
        v.components = Some(out);
    }
    let camera = |n: Node, kind: CameraKind| -> Option<BcfCamera> {
        Some(BcfCamera {
            kind,
            viewpoint: point_in(n, "CameraViewPoint")?,
            direction: point_in(n, "CameraDirection")?,
            up: point_in(n, "CameraUpVector")?,
            aspect: num(n, "AspectRatio"),
        })
    };
    if let Some(p) = child(root, "PerspectiveCamera") {
        v.camera = camera(
            p,
            CameraKind::Perspective {
                fov: num(p, "FieldOfView").unwrap_or(60.0),
            },
        );
    } else if let Some(o) = child(root, "OrthogonalCamera") {
        v.camera = camera(
            o,
            CameraKind::Orthogonal {
                scale: num(o, "ViewToWorldScale").unwrap_or(1.0),
            },
        );
    }
    if let Some(l) = child(root, "Lines") {
        for line in children(l, "Line") {
            if let (Some(a), Some(b)) = (point_in(line, "StartPoint"), point_in(line, "EndPoint")) {
                v.lines.push((a, b));
            }
        }
    }
    if let Some(c) = child(root, "ClippingPlanes") {
        for p in children(c, "ClippingPlane") {
            if let (Some(location), Some(direction)) =
                (point_in(p, "Location"), point_in(p, "Direction"))
            {
                v.clipping_planes.push(ClippingPlane {
                    location,
                    direction,
                });
            }
        }
    }
    // 2.x: Bitmap elements (format in a Bitmap child); 3.0: a Bitmaps list (Format).
    let mut bms: Vec<Node> = children(root, "Bitmap").collect();
    if let Some(w) = child(root, "Bitmaps") {
        bms.extend(children(w, "Bitmap"));
    }
    for b in bms {
        let format = text(b, "Format")
            .or_else(|| text(b, "Bitmap"))
            .unwrap_or_default()
            .to_ascii_lowercase();
        if let (Some(location), Some(normal), Some(up)) = (
            point_in(b, "Location"),
            point_in(b, "Normal"),
            point_in(b, "Up"),
        ) {
            v.bitmaps.push(Bitmap {
                format,
                reference: text(b, "Reference").unwrap_or_default(),
                location,
                normal,
                up,
                height: num(b, "Height").unwrap_or(1.0),
            });
        }
    }
    Ok(v)
}
