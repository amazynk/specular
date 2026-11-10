//! Images from files, through the viewer's decoder registry.

use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::UNIX_EPOCH;

use specular_core::CancelToken;
use specular_decode::{DecodeRequest, Registry};

use crate::layout::ItemScale;
use crate::render::{ImageSource, Loaded};

/// What probing a file found.
#[derive(Clone, Debug, PartialEq)]
pub struct Probed {
    /// Displayed size (orientation applied); `None` when the file cannot be opened.
    pub size: Option<(u32, u32)>,
    /// The file name.
    pub name: String,
    /// Modification time, seconds since the Unix epoch.
    pub modified: Option<u64>,
    /// Capture time (EXIF, or a PNG's creation time), seconds since the Unix epoch.
    pub taken: Option<i64>,
    /// How a drawing measures (PDF pages, CAD sheets), for printing to scale.
    pub scale: Option<ItemScale>,
}

impl Probed {
    /// The date a caption gives: the capture time, else the modification time (seconds since
    /// the Unix epoch).
    pub fn date(&self) -> Option<i64> {
        self.taken
            .or_else(|| self.modified.and_then(|s| i64::try_from(s).ok()))
    }
}

/// The coarsest of `scales` whose result still has `long_px` on the long side, or the
/// finest when the image is smaller than that (never upscaled).
pub fn pick_scale(scales: &[u32], long_side: u32, long_px: u32) -> u32 {
    let finest = scales.iter().copied().min().unwrap_or(1).max(1);
    scales
        .iter()
        .copied()
        .filter(|&s| s > 0 && long_side.div_ceil(s) >= long_px)
        .max()
        .unwrap_or(finest)
}

/// Files decoded on demand.
pub struct FileSource {
    registry: Arc<Registry>,
    paths: Vec<PathBuf>,
}

impl std::fmt::Debug for FileSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileSource")
            .field("paths", &self.paths.len())
            .finish()
    }
}

impl FileSource {
    pub fn new(registry: Arc<Registry>, paths: Vec<PathBuf>) -> FileSource {
        FileSource { registry, paths }
    }

    pub fn paths(&self) -> &[PathBuf] {
        &self.paths
    }

    /// Opens every file for its size (headers only).
    pub fn probe(&self) -> Vec<Probed> {
        self.paths.iter().map(|p| self.probe_one(p)).collect()
    }

    fn probe_one(&self, path: &Path) -> Probed {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let modified = std::fs::metadata(path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs());
        let session = self.registry.open_path(path).ok();
        let size = session.as_ref().map(|s| s.info().display_size());
        let scale = session.as_ref().and_then(|s| item_scale(s.as_ref()));
        Probed {
            size,
            name,
            modified,
            taken: specular_thumbs::list::probe_date_taken(path),
            scale,
        }
    }
}

/// How a session's pixels measure on paper when printed as drawn.
pub fn item_scale(s: &dyn specular_decode::Session) -> Option<ItemScale> {
    let ds = s.drawing_scale()?;
    let mm = ds.paper_mm_per_px.filter(|v| v.is_finite() && *v > 0.0)?;
    Some(ItemScale {
        paper_mm_per_px: mm,
        ratio: ds.ratio,
        unit: ds.unit.symbol().to_owned(),
        unit_mm: ds.unit.metres().map_or(1.0, |m| m * 1e3),
    })
}

impl ImageSource for FileSource {
    fn key(&self, index: usize) -> u64 {
        let mut h = std::hash::DefaultHasher::new();
        self.paths.get(index).hash(&mut h);
        h.finish()
    }

    fn load(&self, index: usize, long_px: u32) -> Result<Loaded, String> {
        let path = self.paths.get(index).ok_or("no such item")?;
        let session = self.registry.open_path(path).map_err(|e| e.to_string())?;
        let info = session.info().clone();
        let scale = pick_scale(info.scales, info.width.max(info.height), long_px);
        let raster = session
            .decode(&DecodeRequest::full(scale), &CancelToken::new())
            .map_err(|e| e.to_string())?;
        let lut = info
            .icc_profile
            .as_ref()
            .and_then(|icc| specular_render::icc::build_lut(icc));
        Ok(Loaded {
            raster,
            stored: (info.width, info.height),
            orientation: info.orientation,
            lut,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_choice() {
        let scales = [1, 2, 4, 8];
        assert_eq!(pick_scale(&scales, 6000, 4961), 1);
        assert_eq!(pick_scale(&scales, 6000, 600), 8);
        assert_eq!(pick_scale(&scales, 6000, 1400), 4);
        assert_eq!(pick_scale(&scales, 100, 256), 1);
        assert_eq!(pick_scale(&[1], 5000, 128), 1);
    }

    /// Captions say when a photo was taken, and fall back to the file's time without one.
    #[test]
    fn caption_date_is_the_capture_time() {
        let photo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../specular-thumbs/tests/data/gpsmeta/gps.jpg");
        let src = FileSource::new(Arc::new(Registry::new()), vec![photo]);
        let p = &src.probe()[0];
        // 2021-07-14 09:26:53, the fixture's DateTimeOriginal.
        assert_eq!(p.taken, Some(1_626_254_813));
        assert_eq!(p.date(), p.taken);
        assert_ne!(p.modified.map(|m| m as i64), p.taken);
        let plain = Probed {
            size: None,
            name: "a.png".into(),
            modified: Some(1_000),
            taken: None,
            scale: None,
        };
        assert_eq!(plain.date(), Some(1_000));
    }
}
