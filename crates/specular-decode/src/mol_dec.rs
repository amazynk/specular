//! Molecular structures (PDB, PDBx/mmCIF, small-molecule CIF, MDL SDF/MOL, molecular XYZ,
//! plain or gzip-compressed) via specular-mol.
//!
//! Like a mesh, a structure has no pixel size: the session renders an isometric preview of
//! its default style (cartoon for polymers, ball-and-stick otherwise) whose long side is
//! [`PREVIEW_LONG_SIDE`](crate::mesh_dec::PREVIEW_LONG_SIDE) px, for thumbnails and the plain image view; the app draws the
//! interactive 3D view from [`Session::molecule`].

use crate::{
    DecodeError, DecodeRequest, Decoder, Format, ImageInfo, MoleculeModel, Result, Session,
};
use parking_lot::Mutex;
use specular_core::{CancelToken, Orientation, PixelLayout, Raster, RasterPlane, Source};
use specular_mol::{Limits, MolFormat, Molecule};
use std::sync::Arc;

pub struct MolDecoder(pub MolFormat);

/// The registry format of a molecular format.
pub fn format_of(f: MolFormat) -> Format {
    match f {
        MolFormat::Pdb => Format::Pdb,
        MolFormat::Mmcif => Format::Mmcif,
        MolFormat::Cif => Format::Cif,
        MolFormat::Sdf => Format::Sdf,
        MolFormat::Xyz => Format::MolXyz,
        MolFormat::Mol2 => Format::Mol2,
        MolFormat::Gro => Format::Gro,
    }
}

impl Decoder for MolDecoder {
    fn format(&self) -> Format {
        format_of(self.0)
    }
    fn probe(&self, head: &[u8]) -> bool {
        specular_mol::sniff_file(head) == Some(self.0)
    }
    fn probe_file(&self, data: &[u8]) -> bool {
        specular_mol::sniff_file(data) == Some(self.0)
    }
    /// `.cif` is the small-molecule decoder's: the content tells mmCIF apart when it is
    /// opened. Molecular XYZ shares `.xyz` with point clouds and goes by content alone.
    fn extensions(&self) -> &'static [&'static str] {
        match self.0 {
            MolFormat::Pdb => &["pdb", "ent", "pdb.gz", "ent.gz"],
            MolFormat::Mmcif => &["mmcif", "mcif", "mmcif.gz"],
            MolFormat::Cif => &["cif", "cif.gz"],
            MolFormat::Sdf => &["sdf", "sd", "mol", "sdf.gz", "mol.gz"],
            MolFormat::Xyz => &[],
            MolFormat::Mol2 => &["mol2", "ml2"],
            MolFormat::Gro => &["gro"],
        }
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        open(self.0, source)
    }
}

fn map_error(e: specular_mol::Error) -> DecodeError {
    match e {
        specular_mol::Error::TooLarge(_) => DecodeError::TooLarge,
        specular_mol::Error::Unsupported(m) => DecodeError::Unsupported(m),
        specular_mol::Error::Malformed(m) => DecodeError::Malformed(m),
        specular_mol::Error::Empty => DecodeError::Malformed("no atoms".into()),
    }
}

/// Opens `source` as a structure: of the format its content says, else of `format` (a file
/// chosen by its extension).
pub(crate) fn open(format: MolFormat, source: Arc<Source>) -> Result<Arc<dyn Session>> {
    let data = source.bytes();
    let found = specular_mol::sniff_file(data).unwrap_or(format);
    let m = specular_mol::open(data, Some(found), &Limits::default()).map_err(map_error)?;
    let (w, h) = crate::mesh_dec::preview_size_of(m.bounds(), specular_mesh::Up::Z);
    let format = m.format.unwrap_or(found);
    let info = ImageInfo {
        format: format_of(format),
        width: w,
        height: h,
        orientation: Orientation::NORMAL,
        icc_profile: None,
        has_alpha: true,
        bit_depth: 8,
        progressive: false,
        coding: coding(&m),
        embedded_preview: None,
        fast_regions: false,
        scales: &[1, 2, 4, 8],
    };
    Ok(Arc::new(MolSession {
        source,
        info,
        model: MoleculeModel {
            molecule: Arc::new(m),
        },
        cache: Mutex::new(None),
    }))
}

/// "structure, 327 atoms, 1 chain" for the status line.
fn coding(m: &Molecule) -> String {
    let mut s = format!("structure, {} atoms", m.atoms.len());
    if !m.chains.is_empty() {
        let n = m.chains.len();
        s = format!("{s}, {n} chain{}", if n == 1 { "" } else { "s" });
    }
    s
}

struct MolSession {
    source: Arc<Source>,
    info: ImageInfo,
    model: MoleculeModel,
    /// The last whole preview rendered, by scale: regions crop it.
    cache: Mutex<Option<(u32, Arc<Vec<u8>>)>>,
}

impl MolSession {
    fn rendered(&self, scale: u32) -> Arc<Vec<u8>> {
        if let Some((s, px)) = &*self.cache.lock()
            && *s == scale
        {
            return px.clone();
        }
        let (w, h) = (
            self.info.width.div_ceil(scale),
            self.info.height.div_ceil(scale),
        );
        let px = Arc::new(specular_mol::preview::render(&self.model.molecule, w, h));
        *self.cache.lock() = Some((scale, px.clone()));
        px
    }
}

impl Session for MolSession {
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
        let full = self.rendered(scale);
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
        // The output, the cached preview, the renderer's supersampled colour and depth, and
        // the tessellated scene (about 1 KiB an atom at most).
        whole * 4 * 2 + whole * 4 * 8 + self.model.molecule.atoms.len() as u64 * 1024
    }
    fn memory_bytes(&self) -> usize {
        let cached = self.cache.lock().as_ref().map_or(0, |(_, px)| px.len());
        self.model.molecule.memory_bytes() + cached
    }
    fn trim_memory(&self) -> usize {
        self.cache.lock().take().map_or(0, |(_, px)| px.len())
    }
    fn molecule(&self) -> Option<&MoleculeModel> {
        Some(&self.model)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Registry;

    fn data(name: &str) -> Vec<u8> {
        let dir = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../formats/specular-mol/tests/data/"
        );
        std::fs::read(format!("{dir}{name}")).expect(name)
    }

    fn open_as(name: &str, bytes: Vec<u8>) -> Result<Arc<dyn Session>> {
        Registry::default().open(Arc::new(Source::from_vec(name, bytes)))
    }

    #[test]
    fn every_format_opens() {
        for (file, format, atoms) in [
            ("1crn.pdb", Format::Pdb, 327),
            ("1crn.pdb.gz", Format::Pdb, 327),
            ("1crn.cif.gz", Format::Mmcif, 327),
            ("cod_1011176.cif", Format::Cif, 0),
            ("aspirin.sdf", Format::Sdf, 21),
            ("caffeine_v3000.mol", Format::Sdf, 24),
            ("caffeine.xyz", Format::MolXyz, 24),
        ] {
            // By content, under a name that says nothing.
            for name in [file, "structure"] {
                let s = open_as(name, data(file)).unwrap_or_else(|e| panic!("{file}: {e}"));
                assert_eq!(s.info().format, format, "{file} as {name}");
                assert!(s.info().format.is_molecule());
                let m = &s.molecule().expect("a structure").molecule;
                if atoms > 0 {
                    assert_eq!(m.atoms.len(), atoms, "{file}");
                }
                assert!(s.mesh().is_none() && s.points().is_none());
            }
        }
    }

    /// `.xyz` holds molecules and point clouds: each opens as what its content is.
    #[test]
    fn xyz_is_told_apart_by_content() {
        let s = open_as("caffeine.xyz", data("caffeine.xyz")).unwrap();
        assert_eq!(s.info().format, Format::MolXyz);
        assert!(s.molecule().is_some() && s.points().is_none());
        let s = open_as("cloud.xyz", data("cloud.xyz")).unwrap();
        assert_eq!(s.info().format, Format::Xyz);
        assert!(s.molecule().is_none() && s.points().is_some());
    }

    #[test]
    fn previews_draw_and_regions_crop() {
        let s = open_as("1crn.pdb", data("1crn.pdb")).unwrap();
        let i = s.info().clone();
        assert_eq!(i.width.max(i.height), crate::mesh_dec::PREVIEW_LONG_SIDE);
        let c = CancelToken::new();
        let full = s.decode(&DecodeRequest::full(4), &c).unwrap();
        let px = &full.planes[0].data;
        assert!(px.chunks_exact(4).filter(|p| p[3] > 0).count() > 1000);
        let mut req = DecodeRequest::full(4);
        req.region = Some(crate::Rect {
            x: 10,
            y: 20,
            w: 30,
            h: 40,
        });
        let part = s.decode(&req, &c).unwrap();
        assert_eq!((part.width, part.height, part.origin), (30, 40, (10, 20)));
        let stride = full.planes[0].stride;
        assert_eq!(
            part.planes[0].data[..30 * 4],
            px[20 * stride + 10 * 4..20 * stride + 40 * 4]
        );
        assert!(s.memory_bytes() > 0);
        assert!(s.trim_memory() > 0);
    }

    #[test]
    fn junk_is_an_error() {
        for name in ["junk.pdb", "junk.cif", "junk.sdf", "junk.pdb.gz"] {
            assert!(
                open_as(name, b"not a structure at all\n".to_vec()).is_err(),
                "{name}"
            );
        }
        // Truncated gzip.
        let gz = data("1crn.pdb.gz");
        assert!(open_as("1crn.pdb.gz", gz[..gz.len() / 2].to_vec()).is_err());
    }
}
