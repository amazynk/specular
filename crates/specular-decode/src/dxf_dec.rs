//! DXF drawings and HP-GL plots (own parsers, `specular-cad`), drawn by `specular-vector`.
//!
//! Model space is page 1 and each layout a page after it; a plot's pages are its pages. The session's pixels are the
//! sheet rasterised on the CPU at a fixed size ([`crate::drawing::LONG_SIDE`] on the long
//! side): what thumbnails, the overview and the first frame show. The viewer draws the drawing itself
//! on the GPU at the exact zoom from [`Session::drawing`], so it is sharp at any zoom.

use std::sync::Arc;

use specular_core::{CancelToken, Orientation, PixelLayout, Raster, RasterPlane, Source};
use specular_vector::draw::View;
use specular_vector::fonts::Fonts;
use specular_vector::kurbo::Affine;
use specular_vector::prep::{PrepLimits, Prepared, prepare};

use crate::drawing::{DrawingPage, paper_colour};
use crate::{DecodeError, DecodeRequest, Decoder, Format, ImageInfo, PageLabel, Result, Session};

pub struct DxfDecoder;

impl Decoder for DxfDecoder {
    fn format(&self) -> Format {
        Format::Dxf
    }
    fn probe(&self, head: &[u8]) -> bool {
        specular_cad::is_dxf(head)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["dxf"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let t0 = std::time::Instant::now();
        let scene =
            specular_cad::parse(source.bytes(), &specular_cad::Limits::default()).map_err(|e| {
                match e {
                    specular_cad::CadError::NotDxf | specular_cad::CadError::NotHpgl => {
                        DecodeError::UnsupportedFormat
                    }
                    specular_cad::CadError::TooLarge => DecodeError::TooLarge,
                    specular_cad::CadError::Malformed(m) => DecodeError::Malformed(m),
                }
            })?;
        let parse_ms = t0.elapsed().as_secs_f64() * 1e3;
        session(source, scene, Format::Dxf, parse_ms)
    }
}

/// Prepares a parsed drawing for drawing and opens its first sheet.
pub(crate) fn session(
    source: Arc<Source>,
    scene: specular_vector::VectorScene,
    format: Format,
    parse_ms: f64,
) -> Result<Arc<dyn Session>> {
    let t1 = std::time::Instant::now();
    let mut scene = scene;
    // SHX fonts from the drawing's folder, then the user's SHX font folders.
    let enc = scene
        .codepage
        .as_deref()
        .map(specular_cad::codepage_encoding);
    specular_vector::fonts::attach_shx(&mut scene, source.path().parent(), enc);
    let prepared = Arc::new(prepare(
        Arc::new(scene),
        &Fonts::shared(),
        PrepLimits::default(),
    ));
    let prep_ms = t1.elapsed().as_secs_f64() * 1e3;
    specular_log::debug!(
        target: specular_log::DECODE,
        "{} {}: {} items, {} sheets, parsed in {parse_ms:.1} ms, prepared in {prep_ms:.1} ms",
        format.name(),
        source.path().display(),
        prepared.scene.item_count(),
        prepared.sheets.len(),
    );
    if prepared.sheets.is_empty() {
        return Err(DecodeError::Malformed("no sheets".into()));
    }
    Ok(Arc::new(DxfSession::new(
        source,
        prepared,
        format,
        0,
        parse_ms + prep_ms,
    )))
}

/// HP-GL and HP-GL/2 plot files, PCL 5 jobs that carry HP-GL/2 among them.
pub struct HpglDecoder;

impl Decoder for HpglDecoder {
    fn format(&self) -> Format {
        Format::Hpgl
    }
    fn probe(&self, head: &[u8]) -> bool {
        specular_cad::hpgl::is_hpgl(head)
    }
    fn probe_file(&self, data: &[u8]) -> bool {
        self.probe(&data[..data.len().min(specular_cad::hpgl::PROBE_LEN)])
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["plt", "hpgl", "hpg", "hp2", "hgl", "pen"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let t0 = std::time::Instant::now();
        let scene = specular_cad::hpgl::parse(source.bytes(), &specular_cad::Limits::default())
            .map_err(|e| match e {
                specular_cad::CadError::TooLarge => DecodeError::TooLarge,
                specular_cad::CadError::Malformed(m) => DecodeError::Malformed(m),
                _ => DecodeError::UnsupportedFormat,
            })?;
        // Opened by extension: a plot draws something.
        if scene.item_count() == 0 && !self.probe_file(source.bytes()) {
            return Err(DecodeError::UnsupportedFormat);
        }
        let parse_ms = t0.elapsed().as_secs_f64() * 1e3;
        session(source, scene, Format::Hpgl, parse_ms)
    }
}

struct DxfSession {
    source: Arc<Source>,
    info: ImageInfo,
    page: DrawingPage,
}

impl DxfSession {
    fn new(
        source: Arc<Source>,
        prepared: Arc<Prepared>,
        format: Format,
        sheet: usize,
        open_ms: f64,
    ) -> DxfSession {
        let ((w, h), page) = DrawingPage::new(prepared.clone(), sheet, open_ms);
        let scene = &prepared.scene;
        let mut coding = if scene.version.is_empty() {
            scene.format.clone()
        } else {
            format!("{} {}", scene.format, scene.version)
        };
        if let Some(r) = &scene.release {
            coding = format!("{coding} ({r})");
        }
        let info = ImageInfo {
            format,
            width: w,
            height: h,
            orientation: Orientation::NORMAL,
            icc_profile: None,
            has_alpha: false,
            bit_depth: 8,
            progressive: false,
            coding,
            embedded_preview: None,
            fast_regions: true,
            scales: &[1, 2, 4, 8],
        };
        DxfSession { source, info, page }
    }
}

impl Session for DxfSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn alpha(&self) -> Option<crate::Alpha> {
        None
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        if cancel.is_cancelled() {
            return Err(DecodeError::Cancelled);
        }
        let scale = req.scale.clamp(1, 8);
        let (sw, sh) = (
            self.info.width.div_ceil(scale),
            self.info.height.div_ceil(scale),
        );
        let (x, y, w, h) = match req.region {
            Some(r) => {
                let x = r.x.min(sw);
                let y = r.y.min(sh);
                (x, y, r.w.min(sw - x), r.h.min(sh - y))
            }
            None => (0, 0, sw, sh),
        };
        if w == 0 || h == 0 {
            return Err(DecodeError::Unsupported("empty region".into()));
        }
        let sheet = self
            .page
            .prepared
            .sheets
            .get(self.page.sheet)
            .ok_or(DecodeError::Malformed("no sheet".into()))?;
        // Light paper, like a plot: thumbnails and the overview look the same in either
        // theme; the viewer paints the dark paper itself when it draws on the GPU.
        let dark = false;
        let layers: Vec<bool> = self
            .page
            .prepared
            .scene
            .layers
            .iter()
            .map(|l| l.visible())
            .collect();
        let s = 1.0 / f64::from(scale);
        let view = View {
            transform: Affine::translate((-f64::from(x), -f64::from(y)))
                * Affine::scale(s)
                * self.page.to_stored,
            size: (w, h),
            dark,
            background: paper_colour(dark),
            lineweights: false,
            dpi_scale: crate::drawing::raster_line_scale(f64::from(sw.max(sh))),
            layers: &layers,
        };
        let (data, _) =
            specular_vector::raster::render(sheet, &view).ok_or(DecodeError::TooLarge)?;
        if cancel.is_cancelled() {
            return Err(DecodeError::Cancelled);
        }
        Ok(Raster {
            width: w,
            height: h,
            origin: (x, y),
            scale,
            layout: PixelLayout::Rgba,
            planes: vec![RasterPlane {
                width: w,
                height: h,
                stride: w as usize * 4,
                data,
                h_div: 1,
                v_div: 1,
            }],
            truncated: false,
        })
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let s = req.scale.clamp(1, 8);
        let px = match req.region {
            Some(r) => r.w as u64 * r.h as u64,
            None => self.info.width.div_ceil(s) as u64 * self.info.height.div_ceil(s) as u64,
        };
        px * 4 * 2
    }
    fn memory_bytes(&self) -> usize {
        self.page
            .prepared
            .sheets
            .iter()
            .flat_map(|s| s.passes.iter())
            .map(|p| p.geometry.heap_bytes())
            .sum()
    }
    fn page_count(&self) -> u32 {
        self.page.prepared.sheets.len() as u32
    }
    fn page_index(&self) -> u32 {
        self.page.sheet as u32
    }
    fn page_label(&self) -> Option<PageLabel> {
        if self.info.format == Format::Hpgl {
            // Plain page numbers.
            return None;
        }
        let s = self.page.prepared.sheets.get(self.page.sheet)?;
        use specular_vector::Generated as G;
        if let Some(g @ (G::BoardLayers | G::BoardTop | G::BoardBottom)) = s.generated {
            if let Some(step) = &s.group {
                return Some(PageLabel::BoardStep {
                    step: step.as_str().into(),
                    view: g,
                });
            }
            return Some(PageLabel::Generated(g));
        }
        if s.generated.is_some() && s.kind != specular_vector::SheetKind::Model {
            // A name Specular made up ("Page 2"): the page number says it in any language.
            return None;
        }
        Some(PageLabel::Sheet {
            name: s.name.as_str().into(),
            model: s.kind == specular_vector::SheetKind::Model,
        })
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        let p = page as usize;
        if p >= self.page.prepared.sheets.len() {
            return Err(DecodeError::Unsupported("no such page".into()));
        }
        Ok(Arc::new(DxfSession::new(
            self.source.clone(),
            self.page.prepared.clone(),
            self.info.format,
            p,
            0.0,
        )))
    }
    fn drawing(&self) -> Option<&DrawingPage> {
        Some(&self.page)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINE: &str = "0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1009\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nLINE\n8\n0\n10\n0\n20\n0\n11\n100\n21\n50\n0\nENDSEC\n0\nEOF\n";

    #[test]
    fn opens_and_rasterises() {
        let src = Arc::new(Source::from_vec("t.dxf", LINE.as_bytes().to_vec()));
        let reg = crate::Registry::default();
        let s = reg.open(src).expect("opens");
        assert_eq!(s.info().format, Format::Dxf);
        assert_eq!(s.info().width, crate::drawing::LONG_SIDE as u32);
        assert!(s.info().height < s.info().width);
        assert_eq!(s.page_count(), 1);
        let d = s.drawing().expect("a drawing");
        assert_eq!(d.prepared.scene.units.code, 4);
        let r = s
            .decode(&DecodeRequest::full(8), &CancelToken::new())
            .expect("decodes");
        let px = &r.planes[0].data;
        let ink = px.chunks(4).filter(|p| p[0] < 128).count();
        assert!(ink > 100, "{ink}");
    }

    #[test]
    fn plots_open_by_content_and_extension() {
        let reg = crate::Registry::default();
        let plot = b"IN;SP1;PU0,0;PD4000,0,4000,2000;PG;SP2;PU0,0;PD1000,1000;".to_vec();
        let s = reg
            .open(Arc::new(Source::from_vec("t.bin", plot.clone())))
            .expect("opens by content");
        assert_eq!(s.info().format, Format::Hpgl);
        assert_eq!(s.page_count(), 2);
        assert!(s.page_label().is_none());
        let p2 = s.open_page(1).expect("page 2");
        assert_eq!(p2.info().format, Format::Hpgl);
        assert_eq!(p2.page_index(), 1);
        let r = s
            .decode(&DecodeRequest::full(8), &CancelToken::new())
            .expect("decodes");
        // Hairlines along pixel edges: count any ink.
        let ink = r.planes[0].data.chunks(4).filter(|p| p[0] < 230).count();
        assert!(ink > 300, "{ink}");
        // A plot whose head the probe does not settle (a comment line first), by extension.
        let mut odd = b"garbage line\n".to_vec();
        odd.extend_from_slice(&plot);
        assert!(
            reg.open(Arc::new(Source::from_vec("t.bin", odd.clone())))
                .is_err()
        );
        let s = reg
            .open(Arc::new(Source::from_vec("t.plt", odd)))
            .expect("opens by extension");
        assert_eq!(s.info().format, Format::Hpgl);
        // A Gnuplot script with the same extension is not a plot.
        let gp = b"set terminal png\nset output 'a.png'\nplot sin(x) with lines\n".to_vec();
        assert!(reg.open(Arc::new(Source::from_vec("t.plt", gp))).is_err());
    }

    /// About 50 MB of HP-GL/2: eight pens of polylines, circles, hatched rectangles and
    /// labels over A0. Prints how long opening (parse and prepare) and the first view (the
    /// stored picture at 1:1) take.
    ///
    /// `cargo test -p specular-decode --release --lib big_plot -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn big_plot() {
        use std::fmt::Write as _;
        let mut s = String::from("BP;IN;PS47520,33640;NP8;FT3,200,45;");
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        let mut rnd = move |n: u64| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed % n
        };
        let mut k = 0u64;
        while s.len() < 50 << 20 {
            k += 1;
            let _ = write!(s, "SP{};PU{},{};PD", 1 + k % 8, rnd(47000), rnd(33000));
            let (mut x, mut y) = (rnd(47000) as i64, rnd(33000) as i64);
            for _ in 0..40 {
                x = (x + rnd(401) as i64 - 200).clamp(0, 47500);
                y = (y + rnd(401) as i64 - 200).clamp(0, 33600);
                let _ = write!(s, "{x},{y},");
            }
            s.pop();
            s.push(';');
            match k % 50 {
                0 => {
                    let _ = write!(s, "CI{};", 50 + rnd(400));
                }
                1 => {
                    let _ = write!(s, "RR300,200;EA{},{};", x + 300, y + 200);
                }
                2 => {
                    let _ = write!(s, "LBPart {k}\x03;");
                }
                _ => {}
            }
        }
        let bytes = s.into_bytes();
        let reg = crate::Registry::default();
        let t = std::time::Instant::now();
        let session = reg
            .open(Arc::new(Source::from_vec("big.plt", bytes.clone())))
            .expect("opens");
        let open_ms = t.elapsed().as_secs_f64() * 1e3;
        let t = std::time::Instant::now();
        let r = session
            .decode(&DecodeRequest::full(1), &CancelToken::new())
            .expect("decodes");
        let view_ms = t.elapsed().as_secs_f64() * 1e3;
        let t = std::time::Instant::now();
        let _ = session
            .decode(&DecodeRequest::full(8), &CancelToken::new())
            .expect("decodes");
        let small_ms = t.elapsed().as_secs_f64() * 1e3;
        let items = session.drawing().unwrap().prepared.scene.item_count();
        println!(
            "{:.1} MB, {items} items: open {open_ms:.0} ms, first view {}x{} {view_ms:.0} ms, \
             1/8 view {small_ms:.0} ms",
            bytes.len() as f64 / 1048576.0,
            r.width,
            r.height
        );
    }
}
