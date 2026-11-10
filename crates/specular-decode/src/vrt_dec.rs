//! GDAL virtual rasters (`.vrt`, parsed by `specular_gis::vrt`): each band is put together
//! from the windows of other rasters it names, read through the registry (GeoTIFF, other
//! VRTs, any raster Specular opens), nearest neighbour as GDAL does by default. Simple and
//! complex sources (a complex source's no-data and linear scaling) are read; the result is
//! rounded and clamped to the band's data type. Sources are only read from disk, beside the
//! VRT or where it names them: never from a server or GDAL's virtual file systems.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use specular_core::{CancelToken, Source};
use specular_gis::vrt::{self, Vrt};

use crate::{DecodeError, DecodeRequest, Registry, Result, Session};

/// Most sessions of source files kept open at once.
const MAX_OPEN: usize = 64;

/// A VRT and what its bands read.
pub(crate) struct VrtPlan {
    pub vrt: Vrt,
    /// The VRT's own source (its folder resolves relative names; sources nest below it).
    source: Arc<Source>,
    open: Mutex<HashMap<PathBuf, Arc<dyn Session>>>,
}

/// Why a source's file name is not read.
fn refuse_name(f: &str) -> Option<&'static str> {
    let l = f.trim().to_ascii_lowercase();
    if l.starts_with("/vsi") {
        Some("in GDAL's virtual file systems")
    } else if l.contains("://") || l.starts_with("http:") || l.starts_with("https:") {
        Some("on a server")
    } else if l.is_empty() || f.contains('\0') {
        Some("unnamed")
    } else {
        None
    }
}

impl VrtPlan {
    pub(crate) fn open(source: Arc<Source>) -> Result<VrtPlan> {
        let text = std::str::from_utf8(source.bytes())
            .map_err(|_| DecodeError::Malformed("the VRT is not UTF-8".into()))?;
        let vrt = vrt::parse(text.trim_start_matches('\u{feff}')).map_err(|e| {
            if e.contains("not supported") {
                DecodeError::Unsupported(e)
            } else {
                DecodeError::Malformed(e)
            }
        })?;
        if !crate::geo::on_disk(&source) {
            return Err(DecodeError::Unsupported(
                "a VRT's sources are read beside it on disk".into(),
            ));
        }
        if vrt.bands.len() > 4096 {
            return Err(DecodeError::Malformed("too many bands".into()));
        }
        for s in vrt.bands.iter().flat_map(|b| &b.sources) {
            if let Some(why) = refuse_name(&s.file) {
                return Err(DecodeError::Unsupported(format!(
                    "a source {why} ({}) is not read",
                    s.file.trim()
                )));
            }
        }
        Ok(VrtPlan {
            vrt,
            source,
            open: Mutex::new(HashMap::new()),
        })
    }

    fn path_of(&self, s: &vrt::Source) -> PathBuf {
        let f = Path::new(s.file.trim());
        if s.relative || f.is_relative() {
            self.source
                .path()
                .parent()
                .unwrap_or(Path::new("."))
                .join(f)
        } else {
            f.to_path_buf()
        }
    }

    fn session(&self, path: &Path) -> Result<Arc<dyn Session>> {
        if let Some(s) = self.open.lock().ok().and_then(|m| m.get(path).cloned()) {
            return Ok(s);
        }
        let missing =
            || DecodeError::Unsupported(format!("the source {} is missing", path.display()));
        if !std::fs::metadata(path).is_ok_and(|m| m.is_file() || m.is_dir()) {
            return Err(missing());
        }
        let src = Source::open(path)
            .map_err(|_| missing())?
            .within(&self.source);
        let s = Registry::default().open(Arc::new(src))?;
        if let Ok(mut m) = self.open.lock()
            && m.len() < MAX_OPEN
        {
            m.insert(path.to_path_buf(), s.clone());
        }
        Ok(s)
    }

    /// Memory the open sources hold.
    pub(crate) fn memory_bytes(&self) -> usize {
        self.open
            .lock()
            .map_or(0, |m| m.values().map(|s| s.memory_bytes()).sum())
    }

    /// Closes the open sources; the memory they held.
    pub(crate) fn clear(&self) -> usize {
        self.open.lock().map_or(0, |mut m| {
            let n = m.values().map(|s| s.memory_bytes()).sum();
            m.clear();
            n
        })
    }

    /// Band `band` (1-based) of a source session as values.
    fn source_band(
        s: &dyn Session,
        band: u32,
        cancel: &CancelToken,
    ) -> Result<(Arc<crate::science::NdPlane>, u32, u32)> {
        let (w, h) = (s.info().width, s.info().height);
        if let Some(sci) = s.science() {
            let idx = if sci.axes.is_empty() {
                (band == 1).then(Vec::new)
            } else {
                (band <= sci.axes[0].len).then(|| vec![band - 1])
            };
            if let Some(idx) = idx
                && let Some(p) = s.nd_plane(&idx, cancel)
            {
                let p = p?;
                if (p.width, p.height) == (w, h) {
                    return Ok((p, w, h));
                }
            }
        }
        // A picture: its channel (grey sources: the grey).
        let r = s.decode(&DecodeRequest::full(1), cancel)?;
        let r = if r.layout.is_wide() {
            return Err(DecodeError::Unsupported(
                "a source of more than 8 bits without values".into(),
            ));
        } else {
            crate::gainmap::to_rgba8(&r)
        };
        let c = (band as usize - 1).min(3);
        let pl = &r.planes[0];
        let mut values = Vec::with_capacity(r.width as usize * r.height as usize);
        for y in 0..r.height as usize {
            let row = &pl.data[y * pl.stride..];
            values.extend((0..r.width as usize).map(|x| f32::from(row[x * 4 + c])));
        }
        Ok((
            Arc::new(crate::science::NdPlane {
                width: r.width,
                height: r.height,
                values,
                truncated: r.truncated,
            }),
            r.width,
            r.height,
        ))
    }

    /// Band `b` (0-based) put together: values, and whether any source was missing or cut.
    pub(crate) fn band(&self, b: u32, cancel: &CancelToken) -> Result<(Vec<f32>, bool)> {
        let band = self
            .vrt
            .bands
            .get(b as usize)
            .ok_or_else(|| DecodeError::Malformed("no such band".into()))?;
        let (w, h) = (self.vrt.width as usize, self.vrt.height as usize);
        let init = band.nodata.unwrap_or(0.0);
        let mut out = vec![init as f32; w * h];
        let mut bad = false;
        for s in &band.sources {
            if cancel.is_cancelled() {
                return Err(DecodeError::Cancelled);
            }
            let path = self.path_of(s);
            let (plane, sw, sh) = match self
                .session(&path)
                .and_then(|ses| Self::source_band(ses.as_ref(), s.band.max(1), cancel))
            {
                Ok(p) => p,
                Err(DecodeError::Cancelled) => return Err(DecodeError::Cancelled),
                Err(e) => {
                    specular_log::debug!(target: specular_log::DECODE, "VRT source {}: {e}", path.display());
                    bad = true;
                    continue;
                }
            };
            bad |= plane.truncated;
            let src = s
                .src_rect
                .unwrap_or([0.0, 0.0, f64::from(sw), f64::from(sh)]);
            let dst = s
                .dst_rect
                .unwrap_or([0.0, 0.0, f64::from(sw), f64::from(sh)]);
            let xs = vrt::spans(dst[0], dst[2], src[0], src[2], w as u32);
            let ys = vrt::spans(dst[1], dst[3], src[1], src[3], h as u32);
            for &(dy, sy) in &ys {
                if sy < 0 || sy >= i64::from(sh) {
                    continue;
                }
                let srow =
                    &plane.values[sy as usize * sw as usize..(sy as usize + 1) * sw as usize];
                let drow = &mut out[dy as usize * w..(dy as usize + 1) * w];
                for &(dx, sx) in &xs {
                    if sx < 0 || sx >= i64::from(sw) {
                        continue;
                    }
                    let v = f64::from(srow[sx as usize]);
                    if let Some(nd) = s.nodata
                        && (v == nd || (nd.is_nan() && v.is_nan()))
                    {
                        continue;
                    }
                    let v = v * s.scale_ratio + s.scale_offset;
                    drow[dx as usize] = vrt::to_type(&band.data_type, v) as f32;
                }
            }
        }
        if let Some(nd) = band.nodata {
            let nd = vrt::to_type(&band.data_type, nd) as f32;
            for v in &mut out {
                if *v == nd || (nd.is_nan() && v.is_nan()) {
                    *v = f32::NAN;
                }
            }
        }
        Ok((out, bad))
    }
}
