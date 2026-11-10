//! GDSII stream files (IC layouts; own reader, `specular-gds`), drawn by `specular-vector`
//! like a DXF drawing: a page per top cell, the cells prepared once and drawn instanced,
//! layer/datatype pairs as layers. OASIS layouts are recognised and refused.

use std::sync::Arc;

use specular_core::Source;
use specular_gds::GdsError;

use crate::{DecodeError, Decoder, Format, Result, Session};

pub struct GdsDecoder;

impl Decoder for GdsDecoder {
    fn format(&self) -> Format {
        Format::Gdsii
    }
    fn probe(&self, head: &[u8]) -> bool {
        specular_gds::is_gds(head) || specular_gds::is_oasis(head)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["gds", "gds2", "gdsii"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let t0 = std::time::Instant::now();
        let lib =
            specular_gds::parse(source.bytes(), specular_gds::Limits::default()).map_err(|e| {
                match e {
                    GdsError::NotGds => DecodeError::UnsupportedFormat,
                    GdsError::Oasis => DecodeError::Unsupported(e.to_string()),
                    GdsError::Truncated(_) => DecodeError::Truncated(e.to_string()),
                    GdsError::Malformed(m) => DecodeError::Malformed(m),
                    GdsError::TooLarge(_) => DecodeError::TooLarge,
                }
            })?;
        let scene = specular_gds::scene::to_scene(&lib);
        drop(lib);
        let parse_ms = t0.elapsed().as_secs_f64() * 1e3;
        crate::dxf_dec::session(source, scene, Format::Gdsii, parse_ms)
    }
}

#[cfg(test)]
mod tests {
    use specular_core::CancelToken;

    use super::*;
    use crate::{DecodeRequest, PageLabel};

    fn fixture(name: &str) -> Arc<Source> {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../formats/specular-gds/tests/data")
            .join(name);
        Arc::new(Source::from_vec(name, std::fs::read(p).expect("fixture")))
    }

    #[test]
    fn opens_layouts_with_a_page_per_top_cell() {
        let reg = crate::Registry::default();
        let s = reg.open(fixture("basic.gds")).expect("opens");
        assert_eq!(s.info().format, Format::Gdsii);
        assert_eq!(s.page_count(), 2);
        let Some(PageLabel::Sheet { name, model }) = s.page_label() else {
            panic!("a sheet label");
        };
        assert!(model);
        let d = s.drawing().expect("a drawing");
        assert_eq!(d.prepared.scene.units.code, 13);
        let layout = d.prepared.scene.layout.as_ref().expect("a layout");
        assert_eq!(layout.info.top_cells.len(), 2);
        assert!(layout.info.top_cells.contains(&name.to_string()));
        let p2 = s.open_page(1).expect("page 2");
        assert_ne!(p2.page_label(), s.page_label());
        let r = s
            .decode(&DecodeRequest::full(4), &CancelToken::new())
            .expect("decodes");
        let ink = r.planes[0]
            .data
            .chunks(4)
            .filter(|p| p[0] != p[1] || p[1] != p[2])
            .count();
        assert!(ink > 100, "{ink}");
    }

    #[test]
    fn sniffs_content_and_refuses_oasis_and_broken_files() {
        let reg = crate::Registry::default();
        // By content whatever the name.
        let mut src = fixture("raw.gds");
        src = Arc::new(Source::from_vec("layout.bin", src.bytes().to_vec()));
        assert_eq!(reg.open(src).expect("opens").info().format, Format::Gdsii);
        match reg.open(fixture("tiny.oas")) {
            Err(DecodeError::Unsupported(m)) => assert!(m.contains("OASIS"), "{m}"),
            r => panic!("{:?}", r.map(|s| s.info().format)),
        }
        assert!(matches!(
            reg.open(fixture("truncated.gds")),
            Err(DecodeError::Truncated(_))
        ));
        assert!(matches!(
            reg.open(fixture("odd.gds")),
            Err(DecodeError::Malformed(_))
        ));
        // A deep hierarchy of billions of flat shapes opens quickly and draws bounded.
        let s = reg.open(fixture("deep.gds")).expect("opens");
        let d = s.drawing().expect("a drawing");
        assert!(
            d.prepared
                .scene
                .warnings
                .iter()
                .any(|w| w.contains("shapes"))
        );
        s.decode(&DecodeRequest::full(1), &CancelToken::new())
            .expect("decodes");
    }

    /// Times opening (parse, cell scene, prepare) and the first view (the stored picture at
    /// 1:1, and at 1/8 as a thumbnail) of the layout named by `SPECULAR_GDS_BIG`:
    ///
    /// `SPECULAR_GDS_BIG=tools/corpus/gdsii/files/caravel/user_project_wrapper.gds cargo test
    /// -p specular-decode --lib big_layout -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn big_layout() {
        let Some(path) = std::env::var_os("SPECULAR_GDS_BIG") else {
            eprintln!("SPECULAR_GDS_BIG not set");
            return;
        };
        let bytes = std::fs::read(&path).expect("reads");
        let mb = bytes.len() as f64 / 1048576.0;
        let reg = crate::Registry::default();
        let t = std::time::Instant::now();
        let s = reg
            .open(Arc::new(Source::from_vec("big.gds", bytes)))
            .expect("opens");
        let open_ms = t.elapsed().as_secs_f64() * 1e3;
        let t = std::time::Instant::now();
        let r = s
            .decode(&DecodeRequest::full(1), &CancelToken::new())
            .expect("decodes");
        let view_ms = t.elapsed().as_secs_f64() * 1e3;
        let t = std::time::Instant::now();
        let _ = s
            .decode(&DecodeRequest::full(8), &CancelToken::new())
            .expect("decodes");
        let small_ms = t.elapsed().as_secs_f64() * 1e3;
        let d = s.drawing().expect("a drawing");
        let info = &d.prepared.scene.layout.as_ref().expect("a layout").info;
        println!(
            "{mb:.1} MB, {} cells, {} shapes, {} flat shapes: open {open_ms:.0} ms, first view \
             {}x{} {view_ms:.0} ms, 1/8 view {small_ms:.0} ms",
            info.cells, info.shapes, info.flat_shapes, r.width, r.height
        );
    }
}
