//! IFC building models (IFC2x3, IFC4, IFC4x3; `.ifc` and `.ifczip`) through Specular's own
//! reader ([`specular_mesh::ifc`]), shown by the mesh viewer. Advanced B-reps (NURBS) are
//! tessellated by OpenCASCADE in the sandboxed decode host when it is installed and on
//! ([`crate::occt_dec`]); otherwise their plane faces are drawn and a note says so. The
//! file's header, the model's facts and its spatial tree (site, building, storeys and what
//! each holds) go to the info panel.
//!
//! The model is built on the loader thread (elements in parallel), never the window's; its
//! memory is reserved from the decode budget for the time it takes.

use std::sync::Arc;

use specular_core::Source;
use specular_mesh::ifc::{BrepTessellator, Tessellation};

use crate::{DecodeError, Format, HeaderFacts, Result, Session};

/// OpenCASCADE as the IFC reader's tessellator.
struct Occt;

impl BrepTessellator for Occt {
    fn tessellate(&self, step: &[u8]) -> std::result::Result<Tessellation, String> {
        let m = crate::occt_dec::tessellate_step(step)?;
        let mut parts: Vec<(String, Vec<[f32; 3]>, Vec<u32>)> = Vec::with_capacity(m.meshes.len());
        for mesh in m.meshes {
            // The product's name ("B123"), whatever the instance adds to it.
            let name = m
                .nodes
                .get(mesh.node as usize)
                .map(|n| {
                    n.name
                        .split([' ', '('])
                        .find(|w| w.starts_with('B'))
                        .unwrap_or("")
                        .to_owned()
                })
                .unwrap_or_default();
            parts.push((name, mesh.positions, mesh.indices));
        }
        Ok(Tessellation {
            parts,
            metres_per_unit: m.unit_metres,
            faces_unmeshed: m.faces_unmeshed,
        })
    }
}

/// Most rows of the spatial tree in the info panel.
const MAX_TREE_ROWS: usize = 300;

pub(crate) fn open(source: Arc<Source>) -> Result<Arc<dyn Session>> {
    let bytes = source.bytes();
    // The index, the parsed elements and the triangles: a few times the file (an `.ifczip`
    // unpacks to several times its size first).
    let factor = if bytes.starts_with(b"PK") { 40 } else { 6 };
    let _hold =
        specular_core::budget::reserve((bytes.len() as u64).saturating_mul(factor).max(64 << 20))?;
    let t0 = std::time::Instant::now();
    let occt = Occt;
    let tess: Option<&dyn BrepTessellator> = if crate::occt_dec::available() {
        Some(&occt)
    } else {
        None
    };
    let model =
        specular_mesh::ifc::read(bytes, &specular_mesh::Limits::default(), tess).map_err(|e| {
            match e {
                specular_mesh::Error::TooLarge(_) => DecodeError::TooLarge,
                specular_mesh::Error::Unsupported(m) => DecodeError::Unsupported(m),
                specular_mesh::Error::Malformed(m) => DecodeError::Malformed(format!("IFC: {m}")),
                specular_mesh::Error::Empty => DecodeError::Unsupported(
                    "this IFC model has no geometry that can be shown".into(),
                ),
            }
        })?;
    specular_log::debug!(
        target: specular_log::DECODE,
        "IFC: {} elements, {} triangles in {:.0} ms",
        model.elements.len(),
        model.mesh.triangle_count(),
        t0.elapsed().as_secs_f64() * 1e3
    );
    let facts = header_facts(&model);
    let mut elements = model.elements;
    elements.sort_by_key(|e| e.first_vertex);
    let index = crate::IfcIndex {
        elements,
        mesh_metres: model.mesh_metres,
        origin_metres: model.origin_metres,
        georef: model.georef,
    };
    let mut mesh = model.mesh;
    mesh.format = Some(specular_mesh::MeshFormat::Ifc);
    Ok(crate::mesh_dec::session_with_ifc(
        source,
        Format::Ifc,
        mesh,
        Some(facts),
        Some(Arc::new(index)),
    ))
}

fn header_facts(m: &specular_mesh::ifc::IfcModel) -> HeaderFacts {
    let get = |k: &str| {
        m.header
            .iter()
            .find(|(n, _)| n == k)
            .map(|(_, v)| v.clone())
            .filter(|v| !v.trim().is_empty())
    };
    let mut summary = Vec::new();
    let creator = [get("Author"), get("Organization")]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(", ");
    if !creator.is_empty() {
        summary.push(("creator", creator));
    }
    if let Some(d) = get("Date") {
        summary.push(("created", d));
    }
    let mut fields: Vec<(String, String)> = m.facts.clone();
    fields.extend(m.header.iter().cloned());
    for (i, (depth, label)) in m.tree.iter().enumerate().take(MAX_TREE_ROWS) {
        fields.push((
            format!("Spatial {}", i + 1),
            format!("{}{label}", "  ".repeat((*depth).min(12))),
        ));
    }
    if m.tree.len() > MAX_TREE_ROWS {
        fields.push((
            "Spatial".to_owned(),
            format!("… {} more", m.tree.len() - MAX_TREE_ROWS),
        ));
    }
    HeaderFacts { summary, fields }
}

#[cfg(test)]
mod tests {
    use crate::{Format, Registry};
    use specular_core::Source;
    use std::sync::Arc;

    fn open(name: &str, bytes: Vec<u8>) -> crate::Result<Arc<dyn crate::Session>> {
        Registry::default().open(Arc::new(Source::from_vec(name, bytes)))
    }

    #[test]
    fn ifc_opens_as_a_mesh_with_its_facts() {
        let s = open("cube.ifc", specular_mesh::fixtures::ifc()).unwrap();
        assert_eq!(s.info().format, Format::Ifc);
        assert!(Format::Ifc.is_mesh());
        let m = s.mesh().expect("a mesh");
        assert_eq!(m.mesh.triangle_count(), 12);
        assert_eq!(m.mesh.unit, specular_mesh::Unit::Metre);
        let h = s.header().expect("facts");
        let field = |k: &str| {
            h.fields
                .iter()
                .find(|(n, _)| n == k)
                .map(|(_, v)| v.as_str())
        };
        assert_eq!(field("Schema"), Some("IFC4"));
        assert_eq!(field("Project"), Some("Cube"));
        assert_eq!(field("Drawn"), Some("1 other element"));
        assert_eq!(field("Spatial 1"), Some("Cube (project)"));
        assert_eq!(field("View definition"), Some("ReferenceView"));
        assert!(
            h.summary
                .iter()
                .any(|(k, v)| *k == "creator" && v == "A, B")
        );
    }

    fn corpus(rel: &str) -> Option<Vec<u8>> {
        let dir = std::env::var_os("SPECULAR_MODELS_EXTRA_CORPUS")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::PathBuf::from(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../tools/corpus/models-extra"
                ))
            });
        let b = std::fs::read(dir.join(rel)).ok();
        if b.is_none() {
            eprintln!("skipped: no {rel} in the corpus");
        }
        b
    }

    /// Advanced B-reps through OpenCASCADE in the decode host when it can run: the cube's
    /// curved faces are tessellated (more than the plane faces drawn without it).
    #[test]
    fn advanced_breps_through_opencascade() {
        let Some(bytes) = corpus("ifc/ifc43/advanced-geometric-shape/cube-advanced-brep.ifc")
        else {
            return;
        };
        if !crate::occt_dec::available() {
            eprintln!("skipped: OpenCASCADE is not installed");
            return;
        }
        let s = match open("cube-advanced-brep.ifc", bytes) {
            Ok(s) => s,
            Err(e) => panic!("{e}"),
        };
        let h = s.header().unwrap();
        let fact = |k: &str| {
            h.fields
                .iter()
                .find(|(n, _)| n == k)
                .map(|(_, v)| v.clone())
        };
        if fact("Advanced B-reps").is_none() {
            // The host could not run here: the plane faces only, with the note.
            let notes = &s.mesh().unwrap().mesh.notes;
            assert!(
                notes.iter().any(|n| n.contains("advanced B-reps")),
                "{notes:?}"
            );
            let must = std::env::var_os("SPECULAR_REQUIRE_OCCT").is_some_and(|v| v == "1");
            assert!(!must, "OpenCASCADE required: {notes:?}");
            eprintln!("skipped: the decode host did not tessellate ({notes:?})");
            return;
        }
        let m = &s.mesh().unwrap().mesh;
        assert!(m.triangle_count() > 12, "{}", m.triangle_count());
        // A NURBS basin (periodic B-spline surfaces): drawn within the faceted version's
        // bounds; faces OpenCASCADE cannot mesh are left out with a note.
        let (Some(adv), Some(fac)) = (
            corpus("ifc/ifc43/advanced-geometric-shape/basin-advanced-brep.ifc"),
            corpus("ifc/ifc43/advanced-geometric-shape/basin-faceted-brep.ifc"),
        ) else {
            return;
        };
        let a = open("basin-advanced-brep.ifc", adv).unwrap();
        let f = open("basin-faceted-brep.ifc", fac).unwrap();
        let (ma, mf) = (&a.mesh().unwrap().mesh, &f.mesh().unwrap().mesh);
        let (ba, bf) = (ma.bounds, mf.bounds);
        for k in 0..3 {
            let size = (bf.max[k] - bf.min[k]).max(1e-3);
            assert!(ba.min[k] >= bf.min[k] - size * 0.05, "{ba:?} {bf:?}");
            assert!(ba.max[k] <= bf.max[k] + size * 0.05, "{ba:?} {bf:?}");
        }
        eprintln!(
            "basin: {} triangles; notes {:?}",
            ma.triangle_count(),
            ma.notes
        );
    }

    /// The same model by content (a `.stp` name) and zipped; ifcXML refused.
    #[test]
    fn ifc_by_content_and_zipped() {
        let bytes = specular_mesh::fixtures::ifc();
        let s = open("cube.stp", bytes.clone()).unwrap();
        assert_eq!(s.info().format, Format::Ifc);
        let mut zip = Vec::new();
        {
            use std::io::Write;
            let mut w = zip::ZipWriter::new(std::io::Cursor::new(&mut zip));
            w.start_file(
                "cube.ifc",
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Stored),
            )
            .unwrap();
            w.write_all(&bytes).unwrap();
            w.finish().unwrap();
        }
        let s = open("cube.ifczip", zip).unwrap();
        assert_eq!(s.mesh().unwrap().mesh.triangle_count(), 12);
        let e = open("cube.ifcxml", b"<?xml version=\"1.0\"?><ifcXML/>".to_vec())
            .err()
            .expect("refused")
            .to_string();
        assert!(e.contains("ifcXML"), "{e}");
    }
}
