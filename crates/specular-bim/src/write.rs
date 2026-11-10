//! Writes an [`Archive`] as a BCF 2.1 or 3.0 `.bcfzip`, in the schemas' element order.
//!
//! Between versions: 3.0 needs a topic type and status (`Issue`, `Open` when missing), lower
//! case GUIDs, a camera in every viewpoint (one without is kept as its snapshot only) and an
//! aspect ratio (1 when unknown); 2.1 allows a field of view of 45°–60° only (clamped) and
//! needs a visibility in every component list (all visible when missing).

use std::fmt::Write as _;
use std::io::{Cursor, Write};

use zip::write::SimpleFileOptions;

use crate::bcf::*;

/// Escapes text and attribute values.
pub(crate) fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&apos;"),
            // Characters XML 1.0 cannot carry.
            c if (c as u32) < 0x20 && !matches!(c, '\t' | '\n' | '\r') => {}
            c => o.push(c),
        }
    }
    o
}

struct X {
    s: String,
    depth: usize,
}

impl X {
    fn new() -> X {
        X {
            s: String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"),
            depth: 0,
        }
    }

    fn indent(&mut self) {
        for _ in 0..self.depth {
            self.s.push_str("  ");
        }
    }

    fn open(&mut self, tag: &str, attrs: &[(&str, Option<&str>)]) {
        self.indent();
        let _ = write!(self.s, "<{tag}");
        for (k, v) in attrs {
            if let Some(v) = v {
                let _ = write!(self.s, " {k}=\"{}\"", esc(v));
            }
        }
        self.s.push_str(">\n");
        self.depth += 1;
    }

    fn empty(&mut self, tag: &str, attrs: &[(&str, Option<&str>)]) {
        self.indent();
        let _ = write!(self.s, "<{tag}");
        for (k, v) in attrs {
            if let Some(v) = v {
                let _ = write!(self.s, " {k}=\"{}\"", esc(v));
            }
        }
        self.s.push_str("/>\n");
    }

    fn close(&mut self, tag: &str) {
        self.depth -= 1;
        self.indent();
        let _ = writeln!(self.s, "</{tag}>");
    }

    fn leaf(&mut self, tag: &str, text: &str) {
        self.indent();
        let _ = writeln!(self.s, "<{tag}>{}</{tag}>", esc(text));
    }

    fn opt(&mut self, tag: &str, text: Option<&str>) {
        if let Some(t) = text {
            self.leaf(tag, t);
        }
    }

    /// An element the 3.0 schema wants non-empty: left out when empty.
    fn opt_nonempty(&mut self, tag: &str, text: Option<&str>, v3: bool) {
        match text {
            Some(t) if v3 && t.is_empty() => {}
            t => self.opt(tag, t),
        }
    }

    fn point(&mut self, tag: &str, p: [f64; 3]) {
        self.open(tag, &[]);
        for (k, v) in ["X", "Y", "Z"].iter().zip(p) {
            self.leaf(k, &num(v));
        }
        self.close(tag);
    }
}

/// A double as the shortest text that reads back the same.
fn num(v: f64) -> String {
    if v.is_finite() {
        format!("{v}")
    } else {
        "0".into()
    }
}

fn guid(g: &str, v: Version) -> String {
    match v {
        Version::V3_0 => g.to_ascii_lowercase(),
        Version::V2_1 => g.to_owned(),
    }
}

fn b(v: bool) -> &'static str {
    if v { "true" } else { "false" }
}

/// The topic folder's member names for each viewpoint (as given, or made up).
fn names(t: &Topic) -> Vec<(String, Option<String>)> {
    t.viewpoints
        .iter()
        .enumerate()
        .map(|(i, vp)| {
            let file = vp.file.clone().unwrap_or_else(|| {
                if i == 0 {
                    "viewpoint.bcfv".into()
                } else {
                    format!("{}.bcfv", vp.guid)
                }
            });
            let snap = vp.snapshot.as_ref().map(|_| {
                vp.snapshot_name.clone().unwrap_or_else(|| {
                    if i == 0 {
                        "snapshot.png".into()
                    } else {
                        format!("{}.png", vp.guid)
                    }
                })
            });
            (file, snap)
        })
        .collect()
}

/// The `markup.bcf` of `t`.
pub fn markup(t: &Topic, v: Version) -> String {
    let v3 = v == Version::V3_0;
    let mut x = X::new();
    x.open(
        "Markup",
        &[(
            "xmlns:xsi",
            Some("http://www.w3.org/2001/XMLSchema-instance"),
        )],
    );
    if !t.files.is_empty() {
        x.open("Header", &[]);
        if v3 {
            x.open("Files", &[]);
        }
        for f in &t.files {
            let ext = b(f.is_external);
            x.open(
                "File",
                &[
                    ("IfcProject", f.ifc_project.as_deref()),
                    ("IfcSpatialStructureElement", f.ifc_spatial.as_deref()),
                    (if v3 { "IsExternal" } else { "isExternal" }, Some(ext)),
                ],
            );
            x.opt_nonempty("Filename", f.filename.as_deref(), v3);
            x.opt("Date", f.date.as_deref());
            x.opt_nonempty("Reference", f.reference.as_deref(), v3);
            x.close("File");
        }
        if v3 {
            x.close("Files");
        }
        x.close("Header");
    }
    let tg = guid(&t.guid, v);
    let ty = t
        .topic_type
        .clone()
        .or_else(|| v3.then(|| "Issue".to_owned()));
    let st = t.status.clone().or_else(|| v3.then(|| "Open".to_owned()));
    x.open(
        "Topic",
        &[
            ("Guid", Some(&tg)),
            ("ServerAssignedId", t.server_id.as_deref().filter(|_| v3)),
            ("TopicType", ty.as_deref()),
            ("TopicStatus", st.as_deref()),
        ],
    );
    if v3 {
        if !t.reference_links.is_empty() {
            x.open("ReferenceLinks", &[]);
            for l in &t.reference_links {
                x.leaf("ReferenceLink", l);
            }
            x.close("ReferenceLinks");
        }
    } else {
        for l in &t.reference_links {
            x.leaf("ReferenceLink", l);
        }
    }
    let title = if v3 && t.title.is_empty() {
        "(untitled)"
    } else {
        &t.title
    };
    x.leaf("Title", title);
    x.opt_nonempty("Priority", t.priority.as_deref(), v3);
    if let Some(i) = t.index {
        x.leaf("Index", &i.to_string());
    }
    if v3 {
        if !t.labels.is_empty() {
            x.open("Labels", &[]);
            for l in &t.labels {
                x.leaf("Label", l);
            }
            x.close("Labels");
        }
    } else {
        for l in &t.labels {
            x.leaf("Labels", l);
        }
    }
    x.leaf("CreationDate", &t.creation_date);
    let author = if v3 && t.creation_author.is_empty() {
        "unknown"
    } else {
        &t.creation_author
    };
    x.leaf("CreationAuthor", author);
    x.opt("ModifiedDate", t.modified_date.as_deref());
    x.opt_nonempty("ModifiedAuthor", t.modified_author.as_deref(), v3);
    x.opt("DueDate", t.due_date.as_deref());
    x.opt_nonempty("AssignedTo", t.assigned_to.as_deref(), v3);
    x.opt_nonempty("Stage", t.stage.as_deref(), v3);
    x.opt_nonempty("Description", t.description.as_deref(), v3);
    if let Some(s) = &t.bim_snippet {
        x.open(
            "BimSnippet",
            &[
                ("SnippetType", Some(&s.snippet_type)),
                (
                    if v3 { "IsExternal" } else { "isExternal" },
                    Some(b(s.is_external)),
                ),
            ],
        );
        x.leaf("Reference", &s.reference);
        x.leaf("ReferenceSchema", &s.reference_schema);
        x.close("BimSnippet");
    }
    if v3 && !t.documents.is_empty() {
        x.open("DocumentReferences", &[]);
    }
    for d in &t.documents {
        if v3 {
            let g = d
                .guid
                .clone()
                .map_or_else(crate::ids::new_guid, |g| guid(&g, v));
            x.open("DocumentReference", &[("Guid", Some(&g))]);
            match (&d.document_guid, &d.referenced) {
                (Some(dg), _) => x.leaf("DocumentGuid", &guid(dg, v)),
                (None, Some(u)) if !u.is_empty() => x.leaf("Url", u),
                _ => {}
            }
            x.opt_nonempty("Description", d.description.as_deref(), true);
        } else {
            x.open(
                "DocumentReference",
                &[
                    ("Guid", d.guid.as_deref()),
                    ("isExternal", Some(b(d.is_external))),
                ],
            );
            x.opt("ReferencedDocument", d.referenced.as_deref());
            x.opt("Description", d.description.as_deref());
        }
        x.close("DocumentReference");
    }
    if v3 && !t.documents.is_empty() {
        x.close("DocumentReferences");
    }
    if v3 && !t.related.is_empty() {
        x.open("RelatedTopics", &[]);
    }
    for r in &t.related {
        x.empty("RelatedTopic", &[("Guid", Some(&guid(r, v)))]);
    }
    if v3 && !t.related.is_empty() {
        x.close("RelatedTopics");
    }
    let names = names(t);
    let comments = |x: &mut X| {
        for c in &t.comments {
            x.open("Comment", &[("Guid", Some(&guid(&c.guid, v)))]);
            x.leaf("Date", &c.date);
            let author = if v3 && c.author.is_empty() {
                "unknown"
            } else {
                &c.author
            };
            x.leaf("Author", author);
            if !(v3 && c.text.is_empty()) {
                x.leaf("Comment", &c.text);
            }
            if let Some(vp) = &c.viewpoint {
                x.empty("Viewpoint", &[("Guid", Some(&guid(vp, v)))]);
            }
            x.opt("ModifiedDate", c.modified_date.as_deref());
            x.opt_nonempty("ModifiedAuthor", c.modified_author.as_deref(), v3);
            x.close("Comment");
        }
    };
    let viewpoints = |x: &mut X, tag: &str| {
        for (vp, (file, snap)) in t.viewpoints.iter().zip(&names) {
            x.open(tag, &[("Guid", Some(&guid(&vp.guid, v)))]);
            if vp.vis.as_ref().is_some_and(|vi| !v3 || vi.camera.is_some()) {
                x.leaf("Viewpoint", file);
            }
            x.opt("Snapshot", snap.as_deref());
            if let Some(i) = vp.index {
                x.leaf("Index", &i.to_string());
            }
            x.close(tag);
        }
    };
    if v3 {
        if !t.comments.is_empty() {
            x.open("Comments", &[]);
            comments(&mut x);
            x.close("Comments");
        }
        if !t.viewpoints.is_empty() {
            x.open("Viewpoints", &[]);
            viewpoints(&mut x, "ViewPoint");
            x.close("Viewpoints");
        }
        x.close("Topic");
    } else {
        x.close("Topic");
        comments(&mut x);
        viewpoints(&mut x, "Viewpoints");
    }
    x.close("Markup");
    x.s
}

fn component_list(x: &mut X, list: &[Component], v: Version) {
    let v3 = v == Version::V3_0;
    for c in list {
        let has = c.originating_system.is_some() || c.authoring_tool_id.is_some();
        let a = [("IfcGuid", c.ifc_guid.as_deref())];
        if has {
            x.open("Component", &a);
            x.opt_nonempty("OriginatingSystem", c.originating_system.as_deref(), v3);
            x.opt_nonempty("AuthoringToolId", c.authoring_tool_id.as_deref(), v3);
            x.close("Component");
        } else {
            x.empty("Component", &a);
        }
    }
}

fn write_hints(x: &mut X, h: &ViewSetupHints) {
    x.empty(
        "ViewSetupHints",
        &[
            ("SpacesVisible", h.spaces_visible.map(b)),
            ("SpaceBoundariesVisible", h.space_boundaries_visible.map(b)),
            ("OpeningsVisible", h.openings_visible.map(b)),
        ],
    );
}

/// The `.bcfv` of `vi`; `None` when 3.0 cannot carry it (no camera).
pub fn visinfo(vi: &VisInfo, v: Version) -> Option<String> {
    let v3 = v == Version::V3_0;
    if v3 && vi.camera.is_none() {
        return None;
    }
    let mut x = X::new();
    let g = guid(&vi.guid, v);
    x.open(
        "VisualizationInfo",
        &[
            ("Guid", Some(&g)),
            (
                "xmlns:xsi",
                Some("http://www.w3.org/2001/XMLSchema-instance"),
            ),
        ],
    );
    if let Some(c) = &vi.components {
        x.open("Components", &[]);
        if !v3 && let Some(h) = &c.hints {
            write_hints(&mut x, h);
        }
        if !c.selection.is_empty() {
            x.open("Selection", &[]);
            component_list(&mut x, &c.selection, v);
            x.close("Selection");
        }
        let dv = b(c.default_visibility);
        let has_inner = (v3 && c.hints.is_some()) || !c.exceptions.is_empty();
        if has_inner {
            x.open("Visibility", &[("DefaultVisibility", Some(dv))]);
            if v3 && let Some(h) = &c.hints {
                write_hints(&mut x, h);
            }
            if !c.exceptions.is_empty() {
                x.open("Exceptions", &[]);
                component_list(&mut x, &c.exceptions, v);
                x.close("Exceptions");
            }
            x.close("Visibility");
        } else {
            x.empty("Visibility", &[("DefaultVisibility", Some(dv))]);
        }
        let colours: Vec<&Coloring> = c
            .coloring
            .iter()
            .filter(|k| !k.components.is_empty())
            .collect();
        if !colours.is_empty() {
            x.open("Coloring", &[]);
            for k in colours {
                let col = k.color.to_ascii_uppercase();
                x.open("Color", &[("Color", Some(&col))]);
                if v3 {
                    x.open("Components", &[]);
                }
                component_list(&mut x, &k.components, v);
                if v3 {
                    x.close("Components");
                }
                x.close("Color");
            }
            x.close("Coloring");
        }
        x.close("Components");
    }
    if let Some(cam) = &vi.camera {
        let (tag, field, value) = match cam.kind {
            CameraKind::Orthogonal { scale } => ("OrthogonalCamera", "ViewToWorldScale", scale),
            CameraKind::Perspective { fov } => {
                let f = if v3 {
                    fov.clamp(1e-3, 179.999)
                } else {
                    fov.clamp(45.0, 60.0)
                };
                ("PerspectiveCamera", "FieldOfView", f)
            }
        };
        x.open(tag, &[]);
        x.point("CameraViewPoint", cam.viewpoint);
        x.point("CameraDirection", cam.direction);
        x.point("CameraUpVector", cam.up);
        x.leaf(field, &num(value));
        if v3 {
            let a = cam
                .aspect
                .filter(|a| a.is_finite() && *a > 0.0)
                .unwrap_or(1.0);
            x.leaf("AspectRatio", &num(a));
        }
        x.close(tag);
    }
    if !vi.lines.is_empty() {
        x.open("Lines", &[]);
        for (a, bb) in &vi.lines {
            x.open("Line", &[]);
            x.point("StartPoint", *a);
            x.point("EndPoint", *bb);
            x.close("Line");
        }
        x.close("Lines");
    }
    if !vi.clipping_planes.is_empty() {
        x.open("ClippingPlanes", &[]);
        for p in &vi.clipping_planes {
            x.open("ClippingPlane", &[]);
            x.point("Location", p.location);
            x.point("Direction", p.direction);
            x.close("ClippingPlane");
        }
        x.close("ClippingPlanes");
    }
    if !vi.bitmaps.is_empty() {
        if v3 {
            x.open("Bitmaps", &[]);
        }
        for bm in &vi.bitmaps {
            let f = if bm.format.eq_ignore_ascii_case("jpg")
                || bm.format.eq_ignore_ascii_case("jpeg")
            {
                "jpg"
            } else {
                "png"
            };
            x.open("Bitmap", &[]);
            if v3 {
                x.leaf("Format", f);
            } else {
                x.leaf("Bitmap", &f.to_ascii_uppercase());
            }
            x.leaf("Reference", &bm.reference);
            x.point("Location", bm.location);
            x.point("Normal", bm.normal);
            x.point("Up", bm.up);
            x.leaf("Height", &num(bm.height));
            x.close("Bitmap");
        }
        if v3 {
            x.close("Bitmaps");
        }
    }
    x.close("VisualizationInfo");
    Some(x.s)
}

/// `bcf.version`.
pub fn version_file(v: Version) -> String {
    let mut x = X::new();
    match v {
        Version::V2_1 => {
            x.open(
                "Version",
                &[
                    ("VersionId", Some("2.1")),
                    (
                        "xmlns:xsi",
                        Some("http://www.w3.org/2001/XMLSchema-instance"),
                    ),
                ],
            );
            x.leaf("DetailedVersion", "2.1");
            x.close("Version");
        }
        Version::V3_0 => x.empty(
            "Version",
            &[
                ("VersionId", Some("3.0")),
                (
                    "xmlns:xsi",
                    Some("http://www.w3.org/2001/XMLSchema-instance"),
                ),
            ],
        ),
    }
    x.s
}

/// `project.bcfp`.
pub fn project_file(p: &Project, v: Version) -> String {
    let mut x = X::new();
    let root = match v {
        Version::V2_1 => "ProjectExtension",
        Version::V3_0 => "ProjectInfo",
    };
    x.open(
        root,
        &[(
            "xmlns:xsi",
            Some("http://www.w3.org/2001/XMLSchema-instance"),
        )],
    );
    x.open("Project", &[("ProjectId", Some(&p.id))]);
    x.opt_nonempty("Name", p.name.as_deref(), v == Version::V3_0);
    x.close("Project");
    if v == Version::V2_1 {
        x.leaf("ExtensionSchema", "");
    }
    x.close(root);
    x.s
}

/// BCF 3.0's `extensions.xml`: what the file gives, with every value the topics use.
pub fn extensions_file(a: &Archive) -> String {
    let mut e = a.extensions.clone();
    let add = |list: &mut Vec<String>, v: Option<&str>| {
        if let Some(v) = v.filter(|v| !v.is_empty())
            && !list.iter().any(|x| x == v)
        {
            list.push(v.to_owned());
        }
    };
    for t in &a.topics {
        add(
            &mut e.topic_types,
            Some(t.topic_type.as_deref().unwrap_or("Issue")),
        );
        add(
            &mut e.topic_statuses,
            Some(t.status.as_deref().unwrap_or("Open")),
        );
        add(&mut e.priorities, t.priority.as_deref());
        for l in &t.labels {
            add(&mut e.topic_labels, Some(l));
        }
        add(&mut e.users, Some(&t.creation_author));
        add(&mut e.users, t.assigned_to.as_deref());
        add(&mut e.users, t.modified_author.as_deref());
        for c in &t.comments {
            add(&mut e.users, Some(&c.author));
        }
        add(&mut e.stages, t.stage.as_deref());
        add(
            &mut e.snippet_types,
            t.bim_snippet.as_ref().map(|s| s.snippet_type.as_str()),
        );
    }
    let mut x = X::new();
    x.open(
        "Extensions",
        &[(
            "xmlns:xsi",
            Some("http://www.w3.org/2001/XMLSchema-instance"),
        )],
    );
    for (wrap, item, list) in [
        ("TopicTypes", "TopicType", &e.topic_types),
        ("TopicStatuses", "TopicStatus", &e.topic_statuses),
        ("Priorities", "Priority", &e.priorities),
        ("TopicLabels", "TopicLabel", &e.topic_labels),
        ("Users", "User", &e.users),
        ("SnippetTypes", "SnippetType", &e.snippet_types),
        ("Stages", "Stage", &e.stages),
    ] {
        let list: Vec<&String> = list.iter().filter(|s| !s.is_empty()).collect();
        if list.is_empty() {
            continue;
        }
        x.open(wrap, &[]);
        for i in list {
            x.leaf(item, i);
        }
        x.close(wrap);
    }
    x.close("Extensions");
    x.s
}

/// Every member of the archive as `version`: (name, bytes).
pub fn members(a: &Archive, version: Version) -> Vec<(String, Vec<u8>)> {
    let mut out = vec![("bcf.version".to_owned(), version_file(version).into_bytes())];
    if let Some(p) = &a.project {
        out.push(("project.bcfp".into(), project_file(p, version).into_bytes()));
    }
    if version == Version::V3_0 {
        out.push(("extensions.xml".into(), extensions_file(a).into_bytes()));
    }
    for t in &a.topics {
        let dir = guid(&t.guid, version);
        out.push((format!("{dir}/markup.bcf"), markup(t, version).into_bytes()));
        for (vp, (file, snap)) in t.viewpoints.iter().zip(names(t)) {
            if let Some(vi) = &vp.vis
                && let Some(s) = visinfo(vi, version)
            {
                out.push((format!("{dir}/{file}"), s.into_bytes()));
            }
            if let (Some(name), Some(bytes)) = (snap, &vp.snapshot) {
                out.push((format!("{dir}/{name}"), bytes.clone()));
            }
        }
        for (name, bytes) in &t.extra_files {
            out.push((format!("{dir}/{name}"), bytes.clone()));
        }
    }
    out
}

/// The archive as a `.bcfzip` of `version`.
pub fn write(a: &Archive, version: Version) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for (name, bytes) in members(a, version) {
        // Snapshots are already compressed.
        let o = if name.ends_with(".png") || name.ends_with(".jpg") {
            opts.compression_method(zip::CompressionMethod::Stored)
        } else {
            opts
        };
        if zip.start_file(name, o).is_ok() {
            let _ = zip.write_all(&bytes);
        }
    }
    zip.finish().map(Cursor::into_inner).unwrap_or_default()
}
