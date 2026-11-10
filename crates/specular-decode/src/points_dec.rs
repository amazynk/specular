//! Point clouds (LAS, LAZ and COPC, PCD, XYZ, PTS, E57, and PLY files without faces) via
//! specular-points.
//!
//! Opening reads the header; LAS and LAZ points are then decoded on a `dec-points` thread,
//! published batch by batch for the app's GPU view, which fills in while they load
//! ([`Session::points`]). Like a mesh, a cloud has no pixel size: [`Session::decode`] waits
//! for the points and renders a CPU preview whose long side is [`PREVIEW_LONG_SIDE`] px,
//! for thumbnails and the plain image view.

use crate::{DecodeError, DecodeRequest, Decoder, Format, ImageInfo, PointsModel, Result, Session};
use parking_lot::Mutex;
use specular_core::{CancelToken, Orientation, PixelLayout, Raster, RasterPlane, Source};
use specular_points::{Limits, PointFormat, preview};
use std::sync::Arc;

pub const PREVIEW_LONG_SIDE: u32 = 1024;

pub struct PointsDecoder(pub PointFormat);

/// The registry format of a point cloud format.
pub fn format_of(f: PointFormat) -> Format {
    match f {
        PointFormat::Las => Format::Las,
        PointFormat::Laz => Format::Laz,
        PointFormat::Pcd => Format::Pcd,
        PointFormat::Xyz => Format::Xyz,
        PointFormat::Pts => Format::Pts,
        PointFormat::Ply => Format::Ply,
        PointFormat::E57 => Format::E57,
        PointFormat::Ptx => Format::Ptx,
    }
}

impl Decoder for PointsDecoder {
    fn format(&self) -> Format {
        format_of(self.0)
    }
    /// LAS and LAZ ("LASF", told apart by the point format's compression bits), E57
    /// ("ASTM-E57") and PCD (its header keys) have signatures; XYZ, PTS and PTX go by
    /// extension.
    fn probe(&self, head: &[u8]) -> bool {
        PointFormat::sniff(head) == Some(self.0)
    }
    fn extensions(&self) -> &'static [&'static str] {
        self.0.extensions()
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        open(self.0, source)
    }
}

pub(crate) fn map_error(e: specular_points::Error) -> DecodeError {
    match e {
        specular_points::Error::TooLarge(_) => DecodeError::TooLarge,
        specular_points::Error::Unsupported(m) => DecodeError::Unsupported(m),
        specular_points::Error::Malformed(m) => DecodeError::Malformed(m),
        specular_points::Error::Empty => DecodeError::Malformed("no points".into()),
        specular_points::Error::Cancelled => DecodeError::Cancelled,
    }
}

/// Opens `source` as a cloud of `format`; the points of LAS and LAZ load in the background.
pub(crate) fn open(format: PointFormat, source: Arc<Source>) -> Result<Arc<dyn Session>> {
    let (loading, job) =
        specular_points::open(format, source.bytes(), &Limits::default()).map_err(map_error)?;
    if let Some(job) = job {
        let (src, l) = (source.clone(), loading.clone());
        // A panic ends the loading as an error (the job's own guard); this keeps it to
        // this file and out of the crash log.
        specular_thread::spawn("dec-points", move || {
            let what = format_args!("loading the points of {}", src.path().display());
            let _ = specular_core::isolate::run(what, || job.run(src.bytes(), &l));
        });
    }
    Ok(session_of(source, format_of(format), loading))
}

/// A session showing `loading` (read by this or another reader: 3D Tiles) as `format`.
pub(crate) fn session_of(
    source: Arc<Source>,
    format: Format,
    loading: Arc<specular_points::Loading>,
) -> Arc<dyn Session> {
    let i = &loading.info;
    let aspect = preview::aspect(&loading.bounds);
    let l = PREVIEW_LONG_SIDE as f32;
    let (w, h) = if aspect >= 1.0 {
        (PREVIEW_LONG_SIDE, (l / aspect).round().max(1.0) as u32)
    } else {
        ((l * aspect).round().max(1.0) as u32, PREVIEW_LONG_SIDE)
    };
    let mut coding = format!("point cloud, {} points", i.total);
    if let Some(pf) = i.point_format {
        coding = format!("{coding}, point format {pf}");
    }
    let info = ImageInfo {
        format,
        width: w,
        height: h,
        orientation: Orientation::NORMAL,
        icc_profile: None,
        has_alpha: true,
        bit_depth: 8,
        progressive: false,
        coding,
        embedded_preview: None,
        fast_regions: false,
        scales: &[1, 2, 4, 8],
    };
    Arc::new(PointsSession {
        source,
        info,
        model: PointsModel { loading },
        cache: Mutex::new(None),
    })
}

struct PointsSession {
    source: Arc<Source>,
    info: ImageInfo,
    model: PointsModel,
    /// The last whole preview rendered, by scale: regions crop it.
    cache: Mutex<Option<(u32, Arc<Vec<u8>>)>>,
}

impl Drop for PointsSession {
    fn drop(&mut self) {
        // Nobody can see the points any more (the app's view holds the session).
        if Arc::strong_count(&self.model.loading) <= 2 {
            self.model.loading.cancel();
        }
    }
}

impl PointsSession {
    fn rendered(&self, scale: u32) -> Result<Arc<Vec<u8>>> {
        if let Some((s, px)) = &*self.cache.lock()
            && *s == scale
        {
            return Ok(px.clone());
        }
        let cloud = self.model.loading.wait().map_err(map_error)?;
        let (w, h) = (
            self.info.width.div_ceil(scale),
            self.info.height.div_ceil(scale),
        );
        let px = Arc::new(preview::render(&cloud, w, h));
        *self.cache.lock() = Some((scale, px.clone()));
        Ok(px)
    }
}

impl Session for PointsSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    /// A rendering: transparency is not a property of the file.
    fn alpha(&self) -> Option<crate::Alpha> {
        None
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn scaled_decode_cheap(&self) -> bool {
        true
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
        let full = self.rendered(scale)?;
        if cancel.is_cancelled() {
            return Err(DecodeError::Cancelled);
        }
        let data = if (x, y, w, h) == (0, 0, sw, sh) {
            full.as_ref().clone()
        } else {
            let mut out = Vec::with_capacity(w as usize * h as usize * 4);
            for row in y..y + h {
                let o = (row as usize * sw as usize + x as usize) * 4;
                out.extend_from_slice(&full[o..o + w as usize * 4]);
            }
            out
        };
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
        let s = req.scale.clamp(1, 8) as u64;
        let whole = self.info.width as u64 * self.info.height as u64 / (s * s);
        // The output, the cached whole preview, and the renderer's 2×2 supersampled colour
        // and depth.
        whole * 4 * 2 + whole * 4 * 8
    }
    fn memory_bytes(&self) -> usize {
        let cached = self.cache.lock().as_ref().map_or(0, |(_, px)| px.len());
        let points = match self.model.loading.result() {
            Some(Ok(c)) => c.bytes(),
            _ => self.model.loading.loaded() as usize * 20,
        };
        points + cached
    }
    fn trim_memory(&self) -> usize {
        self.cache.lock().take().map_or(0, |(_, px)| px.len())
    }
    fn points(&self) -> Option<&PointsModel> {
        Some(&self.model)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Registry;

    fn open(name: &str, bytes: Vec<u8>) -> Arc<dyn Session> {
        Registry::default()
            .open(Arc::new(Source::from_vec(name, bytes)))
            .unwrap()
    }

    fn las() -> Vec<u8> {
        specular_points::fixtures::terrain_las(30.0, 10.0)
    }

    #[test]
    fn clouds_open_as_points() {
        let pcd = b"VERSION .7\nFIELDS x y z\nSIZE 4 4 4\nTYPE F F F\nCOUNT 1 1 1\nWIDTH 3\n\
HEIGHT 1\nVIEWPOINT 0 0 0 1 0 0 0\nPOINTS 3\nDATA ascii\n0 0 0\n1 0 0\n0 1 1\n"
            .to_vec();
        let ply = b"ply\nformat ascii 1.0\nelement vertex 3\nproperty float x\n\
property float y\nproperty float z\nend_header\n0 0 0\n1 0 0\n0 1 1\n"
            .to_vec();
        let cases: Vec<(&str, Vec<u8>, Format, usize)> = vec![
            ("terrain.las", las(), Format::Las, 9_000),
            // By signature whatever the name.
            ("terrain.bin", las(), Format::Las, 9_000),
            ("tiny.pcd", pcd, Format::Pcd, 3),
            (
                "tiny.xyz",
                b"0 0 0\n1 0 0\n0 1 1\n".to_vec(),
                Format::Xyz,
                3,
            ),
            ("tiny.ply", ply, Format::Ply, 3),
            // A 2 × 2 PTX scan with one missing return (0 0 0), intensity and colour.
            (
                "scan.ptx",
                b"2\n2\n0 0 0\n1 0 0\n0 1 0\n0 0 1\n1 0 0 0\n0 1 0 0\n0 0 1 0\n10 20 30 1\n\
0 0 0 0.5 0 0 0\n1 0 0 0.25 255 0 0\n0 1 0 0.75 0 255 0\n0 0 1 1 0 0 255\n"
                    .to_vec(),
                Format::Ptx,
                3,
            ),
        ];
        for (name, bytes, format, n) in cases {
            let s = open(name, bytes);
            assert_eq!(s.info().format, format, "{name}");
            assert!(s.mesh().is_none(), "{name}");
            let p = s.points().expect("a cloud");
            assert_eq!(p.loading.wait().unwrap().len(), n, "{name}");
            let r = s
                .decode(&DecodeRequest::full(4), &CancelToken::new())
                .unwrap();
            assert_eq!(r.width, s.info().width.div_ceil(4));
            let a: Vec<u8> = r.planes[0].data.chunks_exact(4).map(|p| p[3]).collect();
            assert!(a.contains(&0) && a.contains(&255), "{name}");
        }
    }

    #[test]
    fn junk_is_an_error_not_a_panic() {
        for name in [
            "x.las", "x.laz", "x.pcd", "x.e57", "x.xyz", "x.pts", "x.ptx",
        ] {
            let r =
                Registry::default().open(Arc::new(Source::from_vec(name, b"nonsense".to_vec())));
            assert!(r.is_err(), "{name}");
        }
        let mut cut = las();
        cut.truncate(400);
        let s = open("cut.las", cut);
        let r = s.decode(&DecodeRequest::full(1), &CancelToken::new());
        // What the cut file holds, at most, or an error; never a panic.
        assert!(r.is_err() || s.points().unwrap().loading.loaded() < 9_000);
    }
}
