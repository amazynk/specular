//! LandXML survey data (`specular-landxml`), drawn by `specular-vector` like a DXF drawing:
//! the plan (surfaces coloured by elevation with contours, alignments with stationing) is
//! page 1; the surfaces and alignments as a 3D model in the mesh view are page 2 (when the
//! file has any); each alignment's profiles are a chart page after them.

use std::sync::Arc;

use specular_core::{CancelToken, Source};

use crate::{
    DecodeError, DecodeRequest, Decoder, Format, ImageInfo, MeshModel, PageLabel, Raster, Result,
    Session,
};

/// A station as surveyors write it ("1+234.50" metric, "12+34.50" imperial).
pub fn station_label(sta: f64, imperial: bool) -> String {
    specular_landxml::align::station_label(sta, imperial, 2)
}

/// The page index of the 3D model, when there is one.
pub const MODEL_PAGE: u32 = 1;

pub struct LandXmlDecoder;

impl Decoder for LandXmlDecoder {
    fn format(&self) -> Format {
        Format::LandXml
    }
    fn probe(&self, head: &[u8]) -> bool {
        specular_landxml::is_landxml(head)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &[]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let t0 = std::time::Instant::now();
        let data = source.bytes();
        if !self.probe(&data[..data.len().min(4096)]) {
            return Err(DecodeError::UnsupportedFormat);
        }
        let x = specular_landxml::parse(data).map_err(|e| match e {
            specular_landxml::Error::NotLandXml => DecodeError::UnsupportedFormat,
            specular_landxml::Error::Malformed(m) => DecodeError::Malformed(m),
            specular_landxml::Error::Unsupported(m) => DecodeError::Unsupported(m),
            specular_landxml::Error::TooLarge => DecodeError::TooLarge,
        })?;
        let scene = specular_landxml::scene(&x);
        let model = specular_landxml::model::model(&x).and_then(|m| mesh_of(m, &x).ok());
        drop(x);
        let parse_ms = t0.elapsed().as_secs_f64() * 1e3;
        let plan = crate::dxf_dec::session(source.clone(), scene, Format::LandXml, parse_ms)?;
        let Some(mesh) = model else {
            return Ok(plan);
        };
        let model = crate::mesh_dec::session(source, Format::LandXml, mesh);
        Ok(Arc::new(WithModel {
            pages: Arc::new(Pages { plan, model }),
            index: 0,
            cur: None,
        }))
    }
}

/// The 3D model as a mesh in the file's unit, z up.
fn mesh_of(
    m: specular_landxml::model::Model,
    x: &specular_landxml::LandXml,
) -> std::result::Result<specular_mesh::Mesh, specular_mesh::Error> {
    let positions = m
        .positions
        .iter()
        .map(|p| [p[0] as f32, p[1] as f32, p[2] as f32])
        .collect();
    let mut mesh = specular_mesh::from_triangles(
        positions,
        m.indices,
        Some(m.colors),
        &specular_mesh::Limits::default(),
    )?;
    let metres = x.units.metres;
    mesh.unit = if (metres - 1.0).abs() < 1e-9 {
        specular_mesh::Unit::Metre
    } else if (metres - 0.3048).abs() < 1e-5 {
        // International and US survey feet differ by 2 ppm.
        specular_mesh::Unit::Foot
    } else {
        specular_mesh::Unit::Unknown
    };
    mesh.up = specular_mesh::Up::Z;
    if m.contours_cut {
        mesh.notes
            .push("Only part of the contours are drawn in 3D.".into());
    }
    Ok(mesh)
}

/// The plan's session (page 0) and the model's.
struct Pages {
    plan: Arc<dyn Session>,
    model: Arc<dyn Session>,
}

/// A LandXML file with a 3D model: the plan's pages with the model as page
/// [`MODEL_PAGE`].
struct WithModel {
    pages: Arc<Pages>,
    index: u32,
    /// The page shown when not the plan's first.
    cur: Option<Arc<dyn Session>>,
}

impl WithModel {
    fn cur(&self) -> &dyn Session {
        self.cur.as_deref().unwrap_or(self.pages.plan.as_ref())
    }
}

impl Session for WithModel {
    fn info(&self) -> &ImageInfo {
        self.cur().info()
    }
    fn source(&self) -> &Arc<Source> {
        self.pages.plan.source()
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        self.cur().decode_preview()
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        self.cur().decode(req, cancel)
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        self.cur().decode_memory(req)
    }
    fn memory_bytes(&self) -> usize {
        self.cur().memory_bytes()
    }
    fn trim_memory(&self) -> usize {
        self.cur().trim_memory()
    }
    fn scaled_decode_cheap(&self) -> bool {
        self.cur().scaled_decode_cheap()
    }
    fn page_count(&self) -> u32 {
        self.pages.plan.page_count() + 1
    }
    fn page_index(&self) -> u32 {
        self.index
    }
    fn page_label(&self) -> Option<PageLabel> {
        if self.index == MODEL_PAGE {
            return Some(PageLabel::Generated(
                crate::drawing::specular_vector::Generated::Model3d,
            ));
        }
        self.cur().page_label()
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        if page >= self.page_count() {
            return Err(DecodeError::Unsupported("no such page".into()));
        }
        let cur = match page {
            0 => None,
            MODEL_PAGE => Some(self.pages.model.clone()),
            p => Some(self.pages.plan.open_page(p - 1)?),
        };
        Ok(Arc::new(WithModel {
            pages: self.pages.clone(),
            index: page,
            cur,
        }))
    }
    fn drawing(&self) -> Option<&crate::drawing::DrawingPage> {
        self.cur().drawing()
    }
    fn mesh(&self) -> Option<&MeshModel> {
        self.cur().mesh()
    }
    fn header(&self) -> Option<&crate::HeaderFacts> {
        self.cur().header()
    }
    fn alpha(&self) -> Option<crate::Alpha> {
        self.cur().alpha()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = r#"<?xml version="1.0"?>
<LandXML xmlns="http://www.landxml.org/schema/LandXML-1.2" version="1.2">
<Units><Metric linearUnit="meter" areaUnit="squareMeter" volumeUnit="cubicMeter"/></Units>
<Surfaces><Surface name="EG"><Definition surfType="TIN"><Pnts>
<P id="1">200 100 10</P><P id="2">200 102 10</P><P id="3">202 102 10</P><P id="4">202 100 10</P>
<P id="5">201 101 11</P></Pnts><Faces><F>1 2 5</F><F>2 3 5</F><F>3 4 5</F><F>4 1 5</F></Faces>
</Definition></Surface></Surfaces>
<Alignments><Alignment name="A" length="2" staStart="0"><CoordGeom>
<Line><Start>201 100</Start><End>201 102</End></Line></CoordGeom></Alignment></Alignments>
</LandXML>"#;

    #[test]
    fn plan_model_and_profile_pages() {
        let s = LandXmlDecoder
            .open(Arc::new(Source::from_vec(
                "s.xml",
                FILE.as_bytes().to_vec(),
            )))
            .unwrap();
        // Plan, 3D model (no profiles in this file).
        assert_eq!(s.page_count(), 2);
        assert!(s.drawing().is_some() && s.mesh().is_none());
        let m = s.open_page(MODEL_PAGE).unwrap();
        assert_eq!(m.page_index(), 1);
        assert_eq!(
            m.page_label(),
            Some(PageLabel::Generated(
                crate::drawing::specular_vector::Generated::Model3d
            ))
        );
        let mesh = &m.mesh().unwrap().mesh;
        assert_eq!(mesh.unit, specular_mesh::Unit::Metre);
        // Centred on the box: the apex at (0, 0, 0.5).
        assert!(mesh.positions.contains(&[0.0, 0.0, 0.5]));
        assert!(mesh.colors.is_some());
        // Back to the plan from the model.
        let p = m.open_page(0).unwrap();
        assert!(p.drawing().is_some() && p.mesh().is_none());
        assert_eq!(p.page_count(), 2);
        assert!(m.open_page(2).is_err());
    }
}
