//! Reflectance transformation images (specular-rti): HP PTM files, CHI HSH `.rti` / `.hsh`
//! files and CNR-ISTI Relight folders (`info.json` with its JPEG planes).
//!
//! A decode is the image relit by a light straight from the viewer (overhead on the
//! hemisphere), which is what thumbnails and the first frame show; the viewer relights the
//! coefficients ([`Session::rti`]) on the GPU as the light moves.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use specular_core::{CancelToken, Orientation, PixelLayout, Raster, Source};
use specular_rti::{Look, Rti};

use crate::whole::{self, Levels};
use crate::{DecodeError, DecodeRequest, Decoder, Format, ImageInfo, Result, Session};

/// PTM, HSH or Relight.
pub(crate) struct RtiDecoder(pub Format);

impl Decoder for RtiDecoder {
    fn format(&self) -> Format {
        self.0
    }
    fn probe(&self, head: &[u8]) -> bool {
        // "PTM_1.x"; HSH text headers and Relight's JSON need more than the first bytes.
        self.0 == Format::Ptm && Rti::probe(head) && head.starts_with(b"PTM_1.")
    }
    fn probe_file(&self, data: &[u8]) -> bool {
        let head = &data[..data.len().min(4096)];
        match self.0 {
            Format::Ptm => head.starts_with(b"PTM_1.") && Rti::probe(head),
            // The text header ("#HSH…" comments, then type 3 and the size); the binary
            // header of old `.hsh` files only by extension.
            Format::Rti => head.first() == Some(&b'#') && Rti::probe(head),
            _ => specular_rti::is_relight_info(data),
        }
    }
    fn extensions(&self) -> &'static [&'static str] {
        match self.0 {
            Format::Ptm => &["ptm"],
            Format::Rti => &["rti", "hsh"],
            // A folder, or its `info.json` (recognised by content).
            _ => &[],
        }
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        Ok(Arc::new(RtiSession::open(source, self.0)?))
    }
}

pub(crate) fn rti_err(e: specular_rti::Error) -> DecodeError {
    use specular_rti::Error as E;
    match e {
        E::NotRti => DecodeError::UnsupportedFormat,
        E::Malformed(s) | E::Jpeg(s) => DecodeError::Malformed(s),
        E::Unsupported(s) => DecodeError::Unsupported(s),
        E::Truncated(s) => DecodeError::Truncated(s.into()),
        E::TooLarge(..) => DecodeError::TooLarge,
        E::Io(name, e) => DecodeError::Io(std::io::Error::new(e.kind(), format!("{name}: {e}"))),
        E::NeedsMemory(n) => DecodeError::NeedsMemory(n),
    }
}

/// The folder of a Relight image: the source itself, or the folder of its `info.json`.
fn relight_dir(source: &Source) -> PathBuf {
    if source.is_folder() {
        source.path().to_path_buf()
    } else {
        source
            .path()
            .parent()
            .map_or_else(PathBuf::new, Path::to_path_buf)
    }
}

/// Reads a Relight image from its folder (only the files `info.json` names).
pub fn read_relight_folder(dir: &Path) -> Result<Rti> {
    let info = std::fs::read(dir.join("info.json"))?;
    read_relight(dir, &info)
}

fn read_relight(dir: &Path, info: &[u8]) -> Result<Rti> {
    Rti::relight(info, &mut |name| {
        std::fs::read(dir.join(name)).map_err(|e| specular_rti::Error::Io(name.into(), e))
    })
    .map_err(rti_err)
}

pub(crate) struct RtiSession {
    source: Arc<Source>,
    info: ImageInfo,
    rti: Arc<Rti>,
    levels: Levels,
    /// The coefficient layers' share of the memory budget, while the session holds them.
    _layers: specular_core::Reservation,
}

/// The coding line of the info panel, e.g. "HSH order 2, RGB" or "PTM LRGB, JPEG".
fn coding(rti: &Rti) -> String {
    let mut s = rti.basis.name().to_owned();
    if let Some(o) = rti.basis.order() {
        s.push_str(&format!(" order {o}"));
    }
    s.push_str(", ");
    s.push_str(rti.colour.name());
    if let specular_rti::Container::Ptm { format, .. } = &rti.container
        && format.contains("JPEG")
    {
        s.push_str(", JPEG");
    }
    s
}

impl RtiSession {
    fn open(source: Arc<Source>, format: Format) -> Result<RtiSession> {
        let rti = match format {
            Format::Relight => {
                let dir = relight_dir(&source);
                if source.is_folder() {
                    read_relight_folder(&dir)?
                } else {
                    read_relight(&dir, source.bytes())?
                }
            }
            _ => Rti::read(source.bytes()).map_err(rti_err)?,
        };
        let layers = specular_core::budget::reserve(rti.layer_bytes())?;
        let info = ImageInfo {
            format,
            width: rti.width,
            height: rti.height,
            orientation: Orientation::NORMAL,
            icc_profile: None,
            has_alpha: false,
            bit_depth: 8,
            progressive: false,
            coding: coding(&rti),
            embedded_preview: None,
            fast_regions: false,
            scales: &[1, 2, 4, 8],
        };
        Ok(RtiSession {
            source,
            info,
            rti: Arc::new(rti),
            levels: Levels::new(),
            _layers: layers,
        })
    }
}

/// The image relit by `look` at 1/`scale` (box-filtered coefficients), as RGBA8.
pub fn relit_raster(rti: &Rti, scale: u32, look: &Look) -> Raster {
    let small;
    let r = if scale > 1 {
        small = rti.downscaled(scale);
        &small
    } else {
        rti
    };
    let mut raster = whole::interleaved(
        r.width,
        r.height,
        PixelLayout::Rgba,
        r.relight_rgba8(look),
        false,
    );
    raster.scale = scale.max(1);
    raster
}

impl Session for RtiSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        self.levels.decode(
            req,
            (self.info.width, self.info.height),
            |s| s,
            |s| Ok(relit_raster(&self.rti, s, &Look::default())),
            cancel,
        )
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        // The relit floats (12 bytes per pixel) and the RGBA result.
        whole::level_bytes(self.info.width, self.info.height, req.scale, 16)
    }
    fn memory_bytes(&self) -> usize {
        self.levels.memory_bytes() + self.rti.memory_bytes()
    }
    fn trim_memory(&self) -> usize {
        self.levels.clear()
    }
    fn scaled_decode_cheap(&self) -> bool {
        false
    }
    fn colour_model(&self) -> Option<&'static str> {
        Some("RGB")
    }
    fn rti(&self) -> Option<Arc<Rti>> {
        Some(self.rti.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixtures() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../formats/specular-rti/tests/data")
    }

    fn registry() -> crate::Registry {
        crate::Registry::new()
    }

    #[test]
    fn files_open_by_content() {
        let reg = registry();
        for (name, format) in [
            ("ptm_lrgb.ptm", Format::Ptm),
            ("ptm_jpeg_rgb.ptm", Format::Ptm),
            ("hsh3.rti", Format::Rti),
            ("legacy.hsh", Format::Rti),
        ] {
            let src = Arc::new(Source::open(fixtures().join(name)).unwrap());
            let s = reg.open(src).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(s.info().format, format, "{name}");
            let rti = s.rti().expect("coefficients");
            assert_eq!((s.info().width, s.info().height), (rti.width, rti.height));
            let r = s
                .decode(&DecodeRequest::full(1), &CancelToken::new())
                .unwrap();
            assert_eq!(r.layout, PixelLayout::Rgba);
            let want = rti.relight_rgba8(&Look::default());
            assert_eq!(r.planes[0].data, want, "{name}");
            let half = s
                .decode(&DecodeRequest::full(2), &CancelToken::new())
                .unwrap();
            assert_eq!(
                (half.width, half.height),
                (rti.width.div_ceil(2), rti.height.div_ceil(2))
            );
        }
        // Content wins over the name: a PTM renamed.
        let bytes = std::fs::read(fixtures().join("ptm_rgb.ptm")).unwrap();
        let src = Arc::new(Source::from_vec("renamed.bin", bytes));
        assert_eq!(reg.open(src).unwrap().info().format, Format::Ptm);
    }

    #[test]
    fn relight_folders_and_their_info_json() {
        let reg = registry();
        let dir = fixtures().join("relight_rbf_mrgb");
        assert!(specular_core::is_image_folder(&dir));
        assert!(reg.supports_file(&dir));
        let s = reg.open(Arc::new(Source::open(&dir).unwrap())).unwrap();
        assert_eq!(s.info().format, Format::Relight);
        assert!(s.info().coding.starts_with("RBF"), "{}", s.info().coding);
        let info = dir.join("info.json");
        assert!(reg.supports_file(&info));
        let s2 = reg.open(Arc::new(Source::open(&info).unwrap())).unwrap();
        assert_eq!(s2.info().format, Format::Relight);
        let a = s
            .decode(&DecodeRequest::full(1), &CancelToken::new())
            .unwrap();
        let b = s2
            .decode(&DecodeRequest::full(1), &CancelToken::new())
            .unwrap();
        assert_eq!(a.planes[0].data, b.planes[0].data);
        // Not every info.json is Relight's.
        assert!(!specular_rti::is_relight_info(
            br#"{"name": "x", "type": "ptm"}"#
        ));
        assert!(!reg.supports_file(&fixtures()));
    }
}
