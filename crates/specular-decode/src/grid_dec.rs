//! Gridded science data through the N-D science path: SRTM elevation tiles and Gaussian
//! cube volumes.
//!
//! * **SRTM** (`.hgt`): 1201 × 1201 (3″) or 3601 × 3601 (1″) big-endian 16-bit heights in
//!   metres, row 0 the northern edge, −32768 for voids (NaN). The tile's name (`N47E011`)
//!   gives its south-west corner, so pixels have longitude and latitude.
//! * **Gaussian cube**: the density's (or each orbital's) grid as a volume, x across, y up
//!   and a z slider in Ångström (Bohr files converted), opening on the middle section; files
//!   with several orbitals (or values) per point get an "Orbital" slider after z. The atoms,
//!   comments and orbital list are header lines.

use std::sync::Arc;

use specular_core::{CancelToken, Orientation, Raster, Source};
use specular_em::cube;

use crate::science::{
    self, AxisKind, Linear, NdAxis, NdPlane, PixelCalibration, PlaneCache, ScienceKind,
    ScienceSummary,
};
use crate::whole::{self, Levels};
use crate::{DecodeError, DecodeRequest, Decoder, Format, ImageInfo, Result, Session};

/// Bytes of the cube header looked at by the probe.
const CUBE_PROBE: usize = 16 << 10;
/// Header lines listed at most (atoms).
const MAX_KEYWORDS: usize = 4096;

/// Gaussian cube (`Format::Cube`) or SRTM (`Format::Srtm`).
pub struct GridDecoder(pub Format);

impl Decoder for GridDecoder {
    fn format(&self) -> Format {
        self.0
    }
    fn probe(&self, head: &[u8]) -> bool {
        // SRTM tiles have no signature: by extension and size only.
        self.0 == Format::Cube && cube::sniff(head)
    }
    fn probe_file(&self, data: &[u8]) -> bool {
        self.probe(&data[..data.len().min(CUBE_PROBE)])
    }
    fn extensions(&self) -> &'static [&'static str] {
        match self.0 {
            // `.cube` is shared with colour lookup tables: claimed by content (`crate::sniff`).
            Format::Srtm => &["hgt"],
            _ => &[],
        }
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let s = match self.0 {
            Format::Srtm => GridSession::srtm(source)?,
            _ => GridSession::cube(source)?,
        };
        Ok(Arc::new(s))
    }
}

/// The south-west corner of an SRTM tile from its name (`N47E011`, `s12w077.hgt`):
/// latitude and longitude in degrees.
pub(crate) fn srtm_corner(name: &str) -> Option<(i32, i32)> {
    let b = name.as_bytes();
    if b.len() < 7 {
        return None;
    }
    let ns = match b[0].to_ascii_uppercase() {
        b'N' => 1,
        b'S' => -1,
        _ => return None,
    };
    let ew = match b[3].to_ascii_uppercase() {
        b'E' => 1,
        b'W' => -1,
        _ => return None,
    };
    let lat: i32 = name.get(1..3)?.parse().ok()?;
    let lon: i32 = name.get(4..7)?.parse().ok()?;
    if lat > 90 || lon > 180 {
        return None;
    }
    Some((ns * lat, ew * lon))
}

/// A cube's atoms as markers over its sections (coloured by element), when the grid's axes
/// are x, y and z (`skew` says they are not). With several sections (`z_axis`, the summary
/// axis of z), each atom shows on the section nearest to it and faded on those either side.
fn cube_atoms(
    c: &cube::Cube,
    path: &std::path::Path,
    skew: bool,
    z_axis: Option<usize>,
) -> Option<Arc<science::AnnotationSet>> {
    use science::annot::{Annotation, Annotations, Format as AnnotFormat, Shape};
    let (sx, sy, sz) = (c.axes[0][0], c.axes[1][1], c.axes[2][2]);
    if c.atoms.is_empty() || skew || sx <= 0.0 || sy <= 0.0 {
        return None;
    }
    let depth = z_axis.filter(|_| sz > 0.0).map(|axis| science::Depth {
        axis,
        at: c
            .atoms
            .iter()
            .map(|a| (a.position[2] - c.origin[2]) / sz)
            .collect(),
    });
    let ny = c.size[1] as f64;
    let items = c
        .atoms
        .iter()
        .map(|a| {
            let x = (a.position[0] - c.origin[0]) / sx;
            let y = (a.position[1] - c.origin[1]) / sy;
            let rgba = specular_mol::element::cpk(a.number.min(255) as u8);
            Annotation {
                name: cube::symbol(a.number).to_owned(),
                group: "Atoms".into(),
                shape: Shape::Point,
                colour: Some([rgba[0], rgba[1], rgba[2]]),
                // Grid point i is pixel i's centre; rows run down from the largest y.
                points: vec![(x + 0.5, ny - 1.0 - y + 0.5)],
            }
        })
        .collect();
    Some(Arc::new(science::AnnotationSet {
        file: path.file_name()?.to_string_lossy().into_owned(),
        set: Annotations {
            format: AnnotFormat::CubeAtoms,
            items,
            skipped: 0,
        },
        depth,
    }))
}

enum Data {
    /// SRTM: samples per side, read from the file per request.
    Srtm(usize),
    /// A cube's values, parsed at open; sections are cut from them per request. The axes of
    /// the summary that are z and the orbital (or value), when there are several.
    Cube {
        cube: Box<cube::Cube>,
        z_axis: Option<usize>,
        value_axis: Option<usize>,
    },
}

struct GridSession {
    source: Arc<Source>,
    info: ImageInfo,
    summary: ScienceSummary,
    data: Data,
    planes: PlaneCache,
    levels: Levels,
    /// A cube's volume for MPR and the 3-D view.
    volume: Option<crate::VolumeFacts>,
}

fn info(format: Format, w: u32, h: u32, bits: u8, coding: String) -> ImageInfo {
    ImageInfo {
        format,
        width: w,
        height: h,
        orientation: Orientation::NORMAL,
        icc_profile: None,
        has_alpha: false,
        bit_depth: bits,
        progressive: false,
        coding,
        embedded_preview: None,
        fast_regions: false,
        scales: &[1, 2, 4, 8],
    }
}

fn kw(k: &str, v: String) -> (String, String, String) {
    (k.to_owned(), v, String::new())
}

impl GridSession {
    fn srtm(source: Arc<Source>) -> Result<GridSession> {
        let n = match source.len() {
            l if l == 2 * 1201 * 1201 => 1201,
            l if l == 2 * 3601 * 3601 => 3601,
            _ => {
                return Err(DecodeError::Unsupported(
                    "not an SRTM tile (1201² or 3601² samples)".into(),
                ));
            }
        };
        let name = source
            .path()
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_owned();
        let mut keywords = vec![kw(
            "Resolution",
            if n == 1201 { "3″" } else { "1″" }.to_owned(),
        )];
        let mut pixel = None;
        if let Some((lat, lon)) = srtm_corner(&name) {
            let step = 1.0 / (n - 1) as f64;
            pixel = Some(PixelCalibration {
                x: Linear {
                    offset: f64::from(lon),
                    step,
                },
                y: Linear {
                    offset: f64::from(lat) + 1.0,
                    step: -step,
                },
                unit_x: "°".into(),
                unit_y: "°".into(),
            });
            keywords.push(kw(
                "South-west corner",
                format!(
                    "{}{:02} {}{:03}",
                    if lat < 0 { 'S' } else { 'N' },
                    lat.abs(),
                    if lon < 0 { 'W' } else { 'E' },
                    lon.abs()
                ),
            ));
        }
        let summary = ScienceSummary {
            width: n as u32,
            height: n as u32,
            sample: "16-bit signed, big-endian".into(),
            unit: Some("m".into()),
            title: (!name.is_empty()).then(|| name.clone()),
            kind: ScienceKind::Gridded,
            pixel,
            keywords: Arc::new(keywords),
            ..Default::default()
        };
        Ok(GridSession {
            info: info(Format::Srtm, n as u32, n as u32, 16, summary.sample.clone()),
            source,
            summary,
            data: Data::Srtm(n),
            planes: PlaneCache::new(),
            levels: Levels::new(),
            volume: None,
        })
    }

    fn cube(source: Arc<Source>) -> Result<GridSession> {
        let c = cube::parse(source.bytes()).map_err(crate::em_dec::em_err)?;
        let [nx, ny, nz] = c.size;
        let per = c.per_point;
        let len = |a: [f64; 3]| (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
        let (sx, sy, sz) = (len(c.axes[0]), len(c.axes[1]), len(c.axes[2]));
        // Ångström to metres for the pixel calibration.
        const M: f64 = 1e-10;
        let pixel = (sx > 0.0 && sy > 0.0).then(|| PixelCalibration {
            x: Linear {
                offset: c.origin[0] * M,
                step: sx * M,
            },
            y: Linear {
                offset: (c.origin[1] + sy * (ny - 1) as f64) * M,
                step: -sy * M,
            },
            unit_x: "m".into(),
            unit_y: "m".into(),
        });
        let mut axes = Vec::new();
        if nz > 1 {
            let lin = Linear {
                offset: c.origin[2],
                step: sz,
            };
            axes.push(NdAxis {
                name: "z".into(),
                len: nz as u32,
                unit: "Å".into(),
                labels: (0..nz)
                    .map(|k| format!("{:.3} Å", lin.at(k as f64)))
                    .collect(),
                wcs_axis: None,
                linear: Some(lin),
                kind: AxisKind::Cube,
            });
        }
        let z_axis = (nz > 1).then_some(0);
        // Several orbitals (or values) per point: one field each.
        let value_axis = (per > 1).then_some(axes.len());
        if per > 1 {
            let labels = (0..per)
                .map(|k| match c.orbitals.get(k) {
                    Some(o) => format!("Orbital {o}"),
                    None => format!("Value {}", k + 1),
                })
                .collect();
            axes.push(NdAxis {
                name: if c.orbitals.is_empty() {
                    "Value".into()
                } else {
                    "Orbital".into()
                },
                len: u32::try_from(per).map_err(|_| DecodeError::TooLarge)?,
                unit: String::new(),
                labels,
                wcs_axis: None,
                linear: None,
                kind: AxisKind::Field,
            });
        }
        let mut keywords = vec![
            kw("Comment", c.comments[0].clone()),
            kw("Comment", c.comments[1].clone()),
            kw(
                "Grid",
                format!("{nx} × {ny} × {nz}, {}", if c.bohr { "Bohr" } else { "Å" }),
            ),
        ];
        let skew = (0..3).any(|a| (0..3).any(|k| a != k && c.axes[a][k].abs() > 1e-9));
        if skew {
            for (a, v) in ["x", "y", "z"].iter().zip(c.axes) {
                keywords.push(kw(
                    &format!("Step {a}"),
                    format!("{:.5} {:.5} {:.5} Å", v[0], v[1], v[2]),
                ));
            }
        }
        if !c.orbitals.is_empty() {
            keywords.push(kw(
                "Orbitals",
                c.orbitals
                    .iter()
                    .map(|o| o.to_string())
                    .collect::<Vec<_>>()
                    .join(" "),
            ));
        }
        keywords.push(kw("Atoms", c.atoms.len().to_string()));
        for (i, a) in c.atoms.iter().enumerate() {
            keywords.push((
                format!("Atom {}", i + 1),
                format!(
                    "{}  {:.4}  {:.4}  {:.4} Å",
                    cube::symbol(a.number),
                    a.position[0],
                    a.position[1],
                    a.position[2]
                ),
                format!("Z = {}, charge {}", a.number, a.charge),
            ));
        }
        keywords.truncate(MAX_KEYWORDS);
        let title = [&c.comments[0], &c.comments[1]]
            .into_iter()
            .find(|s| !s.is_empty())
            .cloned();
        let annotations = cube_atoms(&c, source.path(), skew, z_axis);
        let summary = ScienceSummary {
            annotations,
            width: nx as u32,
            height: ny as u32,
            start: if nz > 1 {
                let mut s = vec![0; axes.len()];
                s[0] = (nz / 2) as u32;
                s
            } else {
                Vec::new()
            },
            axes,
            sample: "decimal text".into(),
            dataset: (per == 1)
                .then(|| c.orbitals.first().map(|o| format!("Orbital {o}")))
                .flatten(),
            title,
            kind: ScienceKind::Gridded,
            pixel,
            keywords: Arc::new(keywords),
            ..Default::default()
        };
        let volume = (nz > 1)
            .then(|| crate::medvol::grids::cube_header(&c).ok())
            .flatten()
            .map(|h| crate::medvol_dec::facts_of(&h));
        Ok(GridSession {
            volume,
            info: info(
                Format::Cube,
                nx as u32,
                ny as u32,
                32,
                summary.sample.clone(),
            ),
            source,
            summary,
            data: Data::Cube {
                cube: Box::new(c),
                z_axis,
                value_axis,
            },
            planes: PlaneCache::new(),
            levels: Levels::new(),
        })
    }

    fn plane(&self, index: &[u32], cancel: &CancelToken) -> Result<Arc<NdPlane>> {
        match &self.data {
            Data::Cube {
                cube: c,
                z_axis,
                value_axis,
            } => {
                let at = |a: &Option<usize>| a.and_then(|a| index.get(a)).copied().unwrap_or(0);
                let (iz, o) = (at(z_axis) as usize, at(value_axis) as usize);
                let [nx, ny, nz] = c.size;
                if iz >= nz || o >= c.per_point {
                    return Err(DecodeError::Malformed("no such plane".into()));
                }
                let key = [iz as u32, o as u32];
                self.planes.get_or(&key, cancel, || {
                    // A section at z: x across, y up (row 0 the largest y).
                    let per = c.per_point;
                    let mut values = vec![0f32; nx * ny];
                    for (row, line) in values.chunks_exact_mut(nx).enumerate() {
                        let iy = ny - 1 - row;
                        for (ix, v) in line.iter_mut().enumerate() {
                            *v = c.values[((ix * ny + iy) * nz + iz) * per + o];
                        }
                    }
                    Ok(NdPlane {
                        width: nx as u32,
                        height: ny as u32,
                        values,
                        truncated: c.truncated,
                    })
                })
            }
            &Data::Srtm(n) => self.planes.get_or(index, cancel, || {
                let values = self
                    .source
                    .bytes()
                    .chunks_exact(2)
                    .take(n * n)
                    .map(|b| match i16::from_be_bytes([b[0], b[1]]) {
                        -32768 => f32::NAN,
                        v => f32::from(v),
                    })
                    .collect();
                Ok(NdPlane {
                    width: n as u32,
                    height: n as u32,
                    values,
                    truncated: false,
                })
            }),
        }
    }

    fn auto(&self, cancel: &CancelToken) -> Result<Raster> {
        let idx = if self.summary.start.len() == self.summary.axes.len() {
            self.summary.start.clone()
        } else {
            vec![0; self.summary.axes.len()]
        };
        Ok(science::auto_raster(&[self.plane(&idx, cancel)?], None))
    }
}

impl Session for GridSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn research_volume(&self) -> Option<&crate::VolumeFacts> {
        self.volume.as_ref()
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
            |_| 1,
            |_| self.auto(cancel),
            cancel,
        )
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let px = u64::from(self.info.width) * u64::from(self.info.height);
        px * 4 + whole::level_bytes(self.info.width, self.info.height, req.scale, 4)
    }
    fn memory_bytes(&self) -> usize {
        let cube = match &self.data {
            Data::Cube { cube, .. } => cube.values.len() * 4,
            Data::Srtm(_) => 0,
        };
        cube + self.levels.memory_bytes() + self.planes.memory_bytes()
    }
    fn trim_memory(&self) -> usize {
        self.levels.clear() + self.planes.clear()
    }
    fn scaled_decode_cheap(&self) -> bool {
        false
    }
    fn colour_model(&self) -> Option<&'static str> {
        Some("Greyscale")
    }
    fn science(&self) -> Option<&ScienceSummary> {
        Some(&self.summary)
    }
    fn nd_plane(&self, index: &[u32], cancel: &CancelToken) -> Option<Result<Arc<NdPlane>>> {
        Some(self.plane(index, cancel))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_names() {
        assert_eq!(srtm_corner("N47E011"), Some((47, 11)));
        assert_eq!(srtm_corner("s12w077"), Some((-12, -77)));
        assert_eq!(srtm_corner("N47X011"), None);
        assert_eq!(srtm_corner("tile"), None);
    }

    fn srtm_bytes(n: usize) -> Vec<u8> {
        let mut d = Vec::with_capacity(2 * n * n);
        for y in 0..n {
            for x in 0..n {
                let v: i16 = if (x, y) == (3, 2) {
                    -32768
                } else {
                    (x as i16 % 1000) - (y as i16 % 500)
                };
                d.extend_from_slice(&v.to_be_bytes());
            }
        }
        d
    }

    #[test]
    fn srtm_tile() {
        let src = Arc::new(Source::from_vec("N47E011.hgt", srtm_bytes(1201)));
        let s = crate::Registry::new().open(src).unwrap();
        assert_eq!(s.info().format, Format::Srtm);
        let sc = s.science().unwrap();
        assert_eq!((sc.width, sc.height), (1201, 1201));
        let px = sc.pixel.as_ref().unwrap();
        assert_eq!(px.x.offset, 11.0);
        assert_eq!(px.y.offset, 48.0);
        assert!((px.y.at(1200.0) - 47.0).abs() < 1e-12);
        let cancel = CancelToken::new();
        let p = s.nd_plane(&[], &cancel).unwrap().unwrap();
        assert_eq!(p.values[0], 0.0);
        assert_eq!(p.values[5], 5.0);
        assert_eq!(p.values[1201 * 7], -7.0);
        assert!(p.values[1201 * 2 + 3].is_nan());
        let r = s.decode(&DecodeRequest::full(4), &cancel).unwrap();
        assert_eq!((r.width, r.height), (301, 301));
        // Other sizes are not SRTM.
        let bad = Arc::new(Source::from_vec("N47E011.hgt", vec![0; 1000]));
        assert!(crate::Registry::new().open(bad).is_err());
    }

    #[test]
    fn cube_volume() {
        let mut t = String::from(" density\n total SCF density\n    2    0.0    0.0    0.0\n");
        t += "    4    0.5    0.0    0.0\n    3    0.0    0.5    0.0\n    5    0.0    0.0    0.5\n";
        t += "    6    6.0    0.0    0.0    0.0\n    1    1.0    1.0    0.0    0.0\n";
        for ix in 0..4 {
            for iy in 0..3 {
                for iz in 0..5 {
                    t += &format!(" {:e}", (100 * ix + 10 * iy + iz) as f64);
                }
                t += "\n";
            }
        }
        let src = Arc::new(Source::from_vec("water.cube", t.into_bytes()));
        let s = crate::Registry::new().open(src).unwrap();
        assert_eq!(s.info().format, Format::Cube);
        let sc = s.science().unwrap();
        assert_eq!((sc.width, sc.height), (4, 3));
        assert_eq!(sc.axes.len(), 1);
        assert_eq!(sc.axes[0].len, 5);
        assert_eq!(sc.start, [2]);
        let lin = sc.axes[0].linear.unwrap();
        assert!((lin.step - 0.5 * cube::BOHR).abs() < 1e-12);
        assert!(
            sc.keywords
                .iter()
                .any(|(k, v, _)| k == "Atom 1" && v.starts_with("C "))
        );
        let cancel = CancelToken::new();
        let p = s.nd_plane(&[2], &cancel).unwrap().unwrap();
        // Row 0 is the largest y (2); x across.
        assert_eq!(&p.values[..4], &[22.0, 122.0, 222.0, 322.0]);
        assert_eq!(&p.values[8..], &[2.0, 102.0, 202.0, 302.0]);
        let px = sc.pixel.as_ref().unwrap();
        assert!((px.x.step - 0.5 * cube::BOHR * 1e-10).abs() < 1e-22);
        s.decode(&DecodeRequest::full(1), &cancel).unwrap();
        // The atoms are markers: carbon at the origin (pixel (0, 0) of the bottom row), the
        // hydrogen two steps along x.
        let a = &sc.annotations.as_ref().unwrap().set.items;
        assert_eq!(a.len(), 2);
        assert_eq!((a[0].name.as_str(), a[0].points[0]), ("C", (0.5, 2.5)));
        assert!((a[1].points[0].0 - 2.5).abs() < 1e-9);
        // Both lie at z = 0: shown on section 0, faded on 1, not on 2 (the start, 2).
        let set = sc.annotations.as_ref().unwrap();
        let d = set.depth.as_ref().unwrap();
        assert_eq!((d.axis, d.at.as_slice()), (0, &[0.0, 0.0][..]));
        assert_eq!(set.opacity(0, &[0]), Some(1.0));
        assert_eq!(set.opacity(1, &[1]), Some(0.35));
        assert_eq!(set.opacity(0, &[2]), None);
        // A volume for MPR.
        assert_eq!(s.research_volume().unwrap().dims, [4, 3, 5]);
    }

    /// Every orbital of a cube is a position on an "Orbital" axis after z.
    #[test]
    fn cube_orbitals() {
        let mut t = String::from(" orbitals\n MO coefficients\n   -1    0.0    0.0    0.0\n");
        t += "    2    1.0    0.0    0.0\n    1    0.0    1.0    0.0\n    3    0.0    0.0    1.0\n";
        t += "    8    8.0    0.0    0.0    0.0\n    2    5    6\n";
        for ix in 0..2 {
            for iz in 0..3 {
                let v = (10 * ix + iz) as f64;
                t += &format!(" {v:e} {:e}", -v);
            }
            t += "\n";
        }
        let src = Arc::new(Source::from_vec("mo.cube", t.into_bytes()));
        let s = crate::Registry::new().open(src).unwrap();
        let sc = s.science().unwrap();
        assert_eq!(sc.axes.len(), 2);
        assert_eq!(sc.axes[1].labels, ["Orbital 5", "Orbital 6"]);
        assert_eq!(sc.start, [1, 0]);
        let cancel = CancelToken::new();
        let a = s.nd_plane(&[2, 0], &cancel).unwrap().unwrap();
        let b = s.nd_plane(&[2, 1], &cancel).unwrap().unwrap();
        assert_eq!(a.values, [2.0, 12.0]);
        assert_eq!(b.values, [-2.0, -12.0]);
        assert!(s.nd_plane(&[0, 2], &cancel).unwrap().is_err());
    }
}
