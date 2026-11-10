//! X3D's XML encoding into the scene graph: an element is a node, its attributes raw field
//! values, its child elements node values of the field their `containerField` names (or the
//! default for their type).

use std::collections::HashMap;

use super::scene::{SNode, Scene, Val};
use crate::Result;
use crate::xml::Node;

/// The field a child node goes into when it does not say.
fn default_container(kind: &str) -> &'static str {
    match kind {
        "Appearance" => "appearance",
        "Material" | "TwoSidedMaterial" | "PhysicalMaterial" | "UnlitMaterial" => "material",
        "ImageTexture" | "PixelTexture" | "MovieTexture" | "MultiTexture" => "texture",
        "TextureTransform" => "textureTransform",
        "Coordinate" | "CoordinateDouble" | "GeoCoordinate" => "coord",
        "Normal" => "normal",
        "Color" | "ColorRGBA" => "color",
        "TextureCoordinate" | "TextureCoordinateGenerator" | "MultiTextureCoordinate" => "texCoord",
        "FontStyle" => "fontStyle",
        "Box"
        | "Sphere"
        | "Cylinder"
        | "Cone"
        | "IndexedFaceSet"
        | "IndexedTriangleSet"
        | "TriangleSet"
        | "IndexedTriangleFanSet"
        | "IndexedTriangleStripSet"
        | "TriangleFanSet"
        | "TriangleStripSet"
        | "IndexedQuadSet"
        | "QuadSet"
        | "ElevationGrid"
        | "Extrusion"
        | "Text"
        | "PointSet"
        | "IndexedLineSet"
        | "LineSet"
        | "Arc2D"
        | "ArcClose2D"
        | "Circle2D"
        | "Disk2D"
        | "Polyline2D"
        | "Polypoint2D"
        | "Rectangle2D"
        | "TriangleSet2D"
        | "NurbsPatchSurface"
        | "NurbsTrimmedSurface"
        | "GeoElevationGrid" => "geometry",
        _ => "children",
    }
}

/// Elements that are not nodes of the scene.
fn ignored(kind: &str) -> bool {
    matches!(
        kind,
        "ROUTE"
            | "IS"
            | "field"
            | "fieldValue"
            | "connect"
            | "head"
            | "meta"
            | "component"
            | "unit"
            | "IMPORT"
            | "EXPORT"
            | "ProtoInterface"
            | "ProtoBody"
            | "Script"
    )
}

const SKIP_ATTRS: [&str; 6] = ["DEF", "USE", "containerField", "class", "id", "style"];

/// Converts the X3D document `root` into `scene`; the root nodes.
pub(crate) fn convert(root: &Node, scene: &mut Scene) -> Result<Vec<u32>> {
    let mut defs: HashMap<String, u32> = HashMap::new();
    if let Some(head) = root.child("head") {
        for m in head.all("meta") {
            match (m.attr("name"), m.attr("content")) {
                (Some("title"), Some(v)) => scene.title = Some(v.to_owned()),
                (Some("generator"), Some(v)) => scene.generator = Some(v.to_owned()),
                _ => {}
            }
        }
    }
    let scene_el = if root.name == "Scene" {
        Some(root)
    } else {
        root.child("Scene")
    };
    let Some(scene_el) = scene_el else {
        return Ok(Vec::new());
    };
    let mut roots = Vec::new();
    for c in &scene_el.children {
        if let Some((_, n)) = element(c, scene, &mut defs)? {
            roots.push(n);
        }
    }
    Ok(roots)
}

/// One element as a node, with the field it goes into.
fn element(
    e: &Node,
    scene: &mut Scene,
    defs: &mut HashMap<String, u32>,
) -> Result<Option<(String, u32)>> {
    let kind = e.name.as_str();
    if ignored(kind) {
        return Ok(None);
    }
    if matches!(kind, "ProtoDeclare" | "ExternProtoDeclare") {
        if let Some(n) = e.attr("name") {
            scene.protos.insert(n.to_owned());
        }
        return Ok(None);
    }
    let field = e
        .attr("containerField")
        .unwrap_or_else(|| default_container(kind))
        .to_owned();
    if let Some(u) = e.attr("USE") {
        return Ok(defs.get(u).map(|&n| (field, n)));
    }
    let mut fields: Vec<(String, Val)> = e
        .attrs
        .iter()
        .filter(|(k, _)| !SKIP_ATTRS.contains(&k.as_str()))
        .map(|(k, v)| (k.clone(), Val::Raw(v.clone())))
        .collect();
    let mut children: Vec<(String, Vec<u32>)> = Vec::new();
    for c in &e.children {
        if let Some((f, n)) = element(c, scene, defs)? {
            match children.iter_mut().find(|(k, _)| *k == f) {
                Some((_, v)) => v.push(n),
                None => children.push((f, vec![n])),
            }
        }
    }
    fields.extend(children.into_iter().map(|(k, v)| (k, Val::Nodes(v))));
    let kind = if kind == "ProtoInstance" {
        // Its declaration's name says which prototype.
        e.attr("name").unwrap_or("ProtoInstance").to_owned()
    } else {
        kind.to_owned()
    };
    if e.name == "ProtoInstance" {
        scene.protos.insert(kind.clone());
    }
    let name = e.attr("DEF").map(str::to_owned);
    let n = scene.add(SNode {
        kind,
        name: name.clone(),
        fields,
    })?;
    if let Some(d) = name {
        defs.insert(d, n);
    }
    Ok(Some((field, n)))
}
