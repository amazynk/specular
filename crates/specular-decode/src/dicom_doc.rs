//! DICOM encapsulated documents: a PDF opens as its pages (through `specular-pdf`), an HL7
//! CDA document as its readable text (set as a PDF) or its embedded PDF, and STL / OBJ models
//! through `specular-mesh`. The document's session is wrapped so the info panel gets the
//! document's title and type and the study summary (no patient attributes); the document's
//! own XMP is not read (it could name the patient).

use std::sync::Arc;

use specular_core::{CancelToken, Raster, Source};
use specular_dicom as dcm;
use specular_dicom::document::{CdaBody, Kind, Style};
use specular_pdf::textdoc;

use crate::dicom_dec::MedicalSummary;
use crate::mesh_dec::MeshDecoder;
use crate::{
    AnimationInfo, DecodeError, DecodeRequest, Decoder, Format, Frames, ImageInfo, MeshModel,
    PageLabel, Result, Session, drawing,
};

/// What the info panel shows about an encapsulated document.
#[derive(Clone, Debug)]
pub struct EncapsulatedFacts {
    /// "PDF", "CDA", "STL", "OBJ".
    pub kind: &'static str,
    pub title: Option<String>,
    pub mime: Option<String>,
    /// The study summary (modality, study and series descriptions; never the patient).
    pub summary: MedicalSummary,
}

/// Opens the encapsulated document of `d` (parsed from `source`).
pub(crate) fn open(
    source: Arc<Source>,
    d: &dcm::Dicom,
    doc: dcm::document::Document,
) -> Result<Arc<dyn Session>> {
    let data = source.bytes();
    let bytes = d
        .buf(data)
        .get(doc.range.clone())
        .ok_or_else(|| DecodeError::Truncated("encapsulated document".into()))?;
    if bytes.is_empty() {
        return Err(DecodeError::Malformed("empty encapsulated document".into()));
    }
    let path = source.path().to_path_buf();
    // The document in place when the data set is not deflated, else a copy.
    let part = || -> Arc<Source> {
        Arc::new(
            Source::from_slice(path.clone(), source.clone(), bytes)
                .unwrap_or_else(|| Source::from_entry(path.clone(), &source, bytes.to_vec())),
        )
    };
    let (inner, kind) = match doc.kind {
        Kind::Pdf => (crate::pdf_dec::open_pdf(part(), Format::Pdf)?, "PDF"),
        Kind::Cda => {
            let pdf = match dcm::document::cda_body(bytes)
                .map_err(|e| DecodeError::Malformed(format!("CDA document: {}", e.0)))?
            {
                CdaBody::Pdf(p) => p,
                CdaBody::Text(lines) => {
                    let lines: Vec<(textdoc::Style, &str)> = lines
                        .iter()
                        .map(|l| {
                            let s = match l.style {
                                Style::Title => textdoc::Style::Title,
                                Style::Heading => textdoc::Style::Heading,
                                Style::Body => textdoc::Style::Body,
                            };
                            (s, l.text.as_str())
                        })
                        .collect();
                    textdoc::build(&lines)
                }
            };
            let s = Arc::new(Source::from_entry(path.clone(), &source, pdf));
            (crate::pdf_dec::open_pdf(s, Format::Pdf)?, "CDA")
        }
        Kind::Stl => (
            MeshDecoder(specular_mesh::MeshFormat::Stl).open(part())?,
            "STL",
        ),
        Kind::Obj => (
            MeshDecoder(specular_mesh::MeshFormat::Obj).open(part())?,
            "OBJ",
        ),
        Kind::Mtl => {
            return Err(DecodeError::Unsupported(
                "DICOM encapsulated MTL (a material library without a model)".into(),
            ));
        }
        Kind::Other => {
            return Err(DecodeError::Unsupported(format!(
                "DICOM encapsulated {} document",
                doc.mime.as_deref().unwrap_or("unknown")
            )));
        }
    };
    let facts = Arc::new(EncapsulatedFacts {
        kind,
        title: doc.title,
        mime: doc.mime,
        summary: d.summary(data),
    });
    Ok(wrap(source, inner, facts))
}

pub(crate) fn wrap(
    source: Arc<Source>,
    inner: Arc<dyn Session>,
    facts: Arc<EncapsulatedFacts>,
) -> Arc<dyn Session> {
    let mut info = inner.info().clone();
    info.coding = format!("DICOM encapsulated {} · {}", facts.kind, info.coding);
    Arc::new(Encapsulated {
        source,
        inner,
        info,
        facts,
    })
}

struct Encapsulated {
    /// The DICOM file.
    source: Arc<Source>,
    inner: Arc<dyn Session>,
    info: ImageInfo,
    facts: Arc<EncapsulatedFacts>,
}

impl Session for Encapsulated {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        self.inner.decode_preview()
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        self.inner.decode(req, cancel)
    }
    fn prepare_regions(&self, cancel: &CancelToken) -> Result<()> {
        self.inner.prepare_regions(cancel)
    }
    fn regions_ready(&self) -> bool {
        self.inner.regions_ready()
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        self.inner.decode_memory(req)
    }
    fn open_memory(&self, levels: &[u32]) -> u64 {
        self.inner.open_memory(levels)
    }
    fn memory_bytes(&self) -> usize {
        self.inner.memory_bytes()
    }
    fn trim_memory(&self) -> usize {
        self.inner.trim_memory()
    }
    fn scaled_decode_cheap(&self) -> bool {
        self.inner.scaled_decode_cheap()
    }
    fn want_levels(&self, scales: &[u32]) {
        self.inner.want_levels(scales);
    }
    fn animation(&self) -> Option<&AnimationInfo> {
        self.inner.animation()
    }
    fn frames(&self, scale: u32) -> Result<Box<dyn Frames>> {
        self.inner.frames(scale)
    }
    fn page_count(&self) -> u32 {
        self.inner.page_count()
    }
    fn page_index(&self) -> u32 {
        self.inner.page_index()
    }
    fn page_label(&self) -> Option<PageLabel> {
        self.inner.page_label()
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        let p = self.inner.open_page(page)?;
        Ok(wrap(self.source.clone(), p, self.facts.clone()))
    }
    fn encapsulated(&self) -> Option<&EncapsulatedFacts> {
        Some(&self.facts)
    }
    fn drawing(&self) -> Option<&drawing::DrawingPage> {
        self.inner.drawing()
    }
    fn mesh(&self) -> Option<&MeshModel> {
        self.inner.mesh()
    }
    fn colour_model(&self) -> Option<&'static str> {
        self.inner.colour_model()
    }
    fn alpha(&self) -> Option<crate::Alpha> {
        self.inner.alpha()
    }
    fn colour_space(&self) -> Option<&'static str> {
        self.inner.colour_space()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Registry;
    use dcm::fixture;

    fn open(data: Vec<u8>) -> Result<Arc<dyn Session>> {
        Registry::default().open(Arc::new(Source::from_vec("doc.dcm", data)))
    }

    fn pdf(pages: usize) -> Vec<u8> {
        let content = specular_pdf::fixture::fill(200.0, 100.0, [0.0, 0.0, 1.0]);
        let page = specular_pdf::fixture::Page {
            width: 200.0,
            height: 100.0,
            content: &content,
        };
        let pages: Vec<_> = (0..pages)
            .map(|_| specular_pdf::fixture::Page {
                content: &content,
                ..page
            })
            .collect();
        specular_pdf::fixture::build(&pages, "", Some("/Author (Doe^Jane)"), "", &[])
    }

    #[test]
    fn encapsulated_pdf_pages_with_document_facts() {
        let file = fixture::document(
            fixture::ENCAPSULATED_PDF,
            "application/pdf",
            Some("Dose report"),
            &pdf(2),
        );
        let s = open(file).unwrap();
        assert_eq!(s.info().format, Format::Pdf);
        assert!(
            s.info().coding.starts_with("DICOM encapsulated PDF"),
            "{}",
            s.info().coding
        );
        assert_eq!(s.page_count(), 2);
        assert!(s.medical().is_none());
        let f = s.encapsulated().unwrap();
        assert_eq!((f.kind, f.title.as_deref()), ("PDF", Some("Dose report")));
        assert_eq!(f.mime.as_deref(), Some("application/pdf"));
        assert_eq!(
            f.summary.study_description.as_deref(),
            Some("Synthetic study")
        );
        assert!(!format!("{f:?}").contains("Test^Patient"));
        assert!(s.embedded_xmp().is_none());
        let c = CancelToken::new();
        let r = s.decode(&DecodeRequest::full(4), &c).unwrap();
        let px = &r.planes[0].data;
        let mid = (r.height as usize / 2) * r.planes[0].stride + r.width as usize / 2 * 4;
        assert_eq!(&px[mid..mid + 3], &[0, 0, 255]);
        let p1 = s.open_page(1).unwrap();
        assert_eq!(p1.page_index(), 1);
        assert!(p1.encapsulated().is_some());
    }

    #[test]
    fn encapsulated_cda_as_text_and_models() {
        let cda = br#"<ClinicalDocument xmlns="urn:hl7-org:v3"><title>Report</title>
            <recordTarget><patientRole><patient><name>Secret</name></patient></patientRole>
            </recordTarget><component><structuredBody><component><section>
            <title>Findings</title><text>Normal.</text></section></component>
            </structuredBody></component></ClinicalDocument>"#;
        let s = open(fixture::document(
            fixture::ENCAPSULATED_CDA,
            "text/XML",
            None,
            cda,
        ))
        .unwrap();
        assert_eq!(s.info().format, Format::Pdf);
        assert_eq!(s.encapsulated().unwrap().kind, "CDA");
        let c = CancelToken::new();
        let r = s.decode(&DecodeRequest::full(4), &c).unwrap();
        assert!(
            r.planes[0].data.chunks(4).any(|p| p[0] < 128),
            "no text drawn"
        );

        let stl = b"solid t\nfacet normal 0 0 1\nouter loop\nvertex 0 0 0\nvertex 1 0 0\n\
                    vertex 0 1 0\nendloop\nendfacet\nendsolid t\n";
        let s = open(fixture::document(
            fixture::ENCAPSULATED_STL,
            "model/stl",
            None,
            stl,
        ))
        .unwrap();
        assert_eq!(s.info().format, Format::Stl);
        assert!(s.mesh().is_some());
        assert_eq!(s.encapsulated().unwrap().kind, "STL");

        let obj = b"v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n";
        let s = open(fixture::document(
            fixture::ENCAPSULATED_OBJ,
            "model/obj",
            None,
            obj,
        ))
        .unwrap();
        assert_eq!(s.info().format, Format::Obj);

        let e = open(fixture::document(
            fixture::ENCAPSULATED_MTL,
            "model/mtl",
            None,
            b"newmtl a\n",
        ))
        .err()
        .unwrap();
        assert!(matches!(e, DecodeError::Unsupported(_)), "{e:?}");
        let e = open(fixture::document("1.2.3", "image/x-nothing", None, b"xx"))
            .err()
            .unwrap();
        assert!(matches!(e, DecodeError::Unsupported(_)), "{e:?}");
    }

    #[test]
    fn damaged_documents_do_not_panic() {
        let file = fixture::document(fixture::ENCAPSULATED_PDF, "application/pdf", None, &pdf(1));
        for cut in (0..file.len()).step_by(41) {
            if let Ok(s) = open(file[..cut].to_vec()) {
                let _ = s.decode(&DecodeRequest::full(8), &CancelToken::new());
            }
        }
        let cda = fixture::document(fixture::ENCAPSULATED_CDA, "text/xml", None, b"<not xml");
        assert!(open(cda).is_err());
    }
}
