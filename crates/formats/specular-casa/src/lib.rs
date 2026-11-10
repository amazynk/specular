//! CASA images: the `.image`, `.residual`, `.model`, `.pb`, `.mask` (and `.psf`, `.sumwt`,
//! `.weight`) directories that CASA's `tclean` and casacore's `PagedImage` write.
//!
//! An image is a casacore table of one row with an array column `map`; we read it with a
//! table reader of our own: [`aipsio`] (the framing of the header files), [`record`]
//! (keyword records), [`table`] (`table.dat`) and [`tsm`] (the TiledCellStMan and
//! TiledShapeStMan storage managers, in either byte order). The `coords` keyword becomes FITS
//! WCS cards ([`coords`]), `imageinfo` gives the restoring beam(s), and the default mask (a
//! Bool subtable, true where valid) blanks pixels.
//!
//! Everything read is bounded: header files are capped, counts are checked against the bytes
//! left, cube sizes against the data files, and planes against [`MAX_ELEMENTS`].

use std::path::{Path, PathBuf};

pub mod aipsio;
pub mod coords;
pub mod record;
pub mod table;
pub mod tsm;

pub use coords::{Axis, AxisKind, Coords, Spectral};
pub use record::{Record, Value};
pub use table::Table;

use record::dtype;

/// The most elements read at once (a plane or a spectrum).
pub const MAX_ELEMENTS: usize = 1 << 31;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum Error {
    #[error("not a CASA image")]
    NotCasa,
    #[error("malformed CASA table: {0}")]
    Malformed(String),
    #[error("unsupported CASA image feature: {0}")]
    Unsupported(String),
    #[error("CASA image data truncated")]
    Truncated,
    #[error("CASA image too large")]
    TooLarge,
    #[error("{0}")]
    Io(String),
    /// The memory budget cannot hold the values.
    #[error("{0}")]
    NeedsMemory(#[from] specular_core::NeedsMemory),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Reads a whole file of at most `max` bytes.
pub(crate) fn read_capped(path: &Path, max: u64) -> Result<Vec<u8>> {
    let meta =
        std::fs::metadata(path).map_err(|e| Error::Io(format!("{}: {e}", path.display())))?;
    if meta.len() > max {
        return Err(Error::TooLarge);
    }
    std::fs::read(path).map_err(|e| Error::Io(format!("{}: {e}", path.display())))
}

/// The image directory of `path`: the directory itself, or the one holding `path` when it
/// is a file of the table (`table.dat`, `table.f0`...).
pub fn image_dir(path: &Path) -> PathBuf {
    if path.is_dir() {
        return path.to_owned();
    }
    path.parent()
        .map_or_else(|| path.to_owned(), Path::to_owned)
}

/// Whether `dir` holds a CASA image: a `table.dat` whose keywords include `coords`.
pub fn is_image_dir(dir: &Path) -> bool {
    use std::io::Read;
    let table = dir.join("table.dat");
    // A regular file only: opening a FIFO blocks.
    if !std::fs::metadata(&table).is_ok_and(|m| m.is_file()) {
        return false;
    }
    let Ok(f) = std::fs::File::open(table) else {
        return false;
    };
    let mut head = Vec::new();
    if f.take(1 << 20).read_to_end(&mut head).is_err() {
        return false;
    }
    sniff(&head)
}

/// Whether `head` (the start of a `table.dat`) describes a CASA image.
pub fn sniff(head: &[u8]) -> bool {
    table::sniff(head) && head.windows(10).any(|w| w == b"\0\0\0\x06coords")
}

/// What the directory name says the image is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageType {
    Image,
    Residual,
    Model,
    PrimaryBeam,
    Mask,
    Psf,
    SumWeight,
    Weight,
    Other,
}

impl ImageType {
    pub fn from_name(dir: &Path) -> ImageType {
        let name = dir
            .file_name()
            .map(|n| n.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        let name = name.trim_end_matches('/');
        // tclean adds `.tt0`, `.tt1` (Taylor terms) after the type.
        let base = name
            .rsplit_once(".tt")
            .filter(|(_, n)| n.parse::<u32>().is_ok())
            .map_or(name, |(b, _)| b);
        match base.rsplit('.').next().unwrap_or("") {
            "image" | "im" | "img" | "pbcor" => ImageType::Image,
            "residual" => ImageType::Residual,
            "model" => ImageType::Model,
            "pb" => ImageType::PrimaryBeam,
            "mask" => ImageType::Mask,
            "psf" => ImageType::Psf,
            "sumwt" => ImageType::SumWeight,
            "weight" => ImageType::Weight,
            _ => ImageType::Other,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            ImageType::Image => "image",
            ImageType::Residual => "residual",
            ImageType::Model => "model",
            ImageType::PrimaryBeam => "pb",
            ImageType::Mask => "mask",
            ImageType::Psf => "psf",
            ImageType::SumWeight => "sumwt",
            ImageType::Weight => "weight",
            ImageType::Other => "other",
        }
    }
}

/// A restoring beam: full widths at half maximum in arcseconds, position angle in degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Beam {
    pub major: f64,
    pub minor: f64,
    pub pa: f64,
}

impl Beam {
    fn from_record(r: &Record) -> Option<Beam> {
        let arcsec = |u: &str| coords::degrees_per(u).map(|d| d * 3600.0);
        let major = coords::quantity(r.record("major")?, arcsec)?;
        let minor = coords::quantity(r.record("minor")?, arcsec)?;
        let pa = r
            .record("positionangle")
            .and_then(|p| coords::quantity(p, coords::degrees_per))
            .unwrap_or(0.0);
        (major > 0.0 && minor >= 0.0).then_some(Beam { major, minor, pa })
    }
}

/// Beams per plane: one per (channel, Stokes), channel fastest.
#[derive(Clone, Debug, PartialEq)]
pub struct PlaneBeams {
    pub channels: usize,
    pub stokes: usize,
    pub beams: Vec<Beam>,
}

impl PlaneBeams {
    pub fn get(&self, channel: usize, stokes: usize) -> Option<Beam> {
        let c = if self.channels == 1 { 0 } else { channel };
        let s = if self.stokes == 1 { 0 } else { stokes };
        if c >= self.channels || s >= self.stokes {
            return None;
        }
        self.beams.get(c + s * self.channels).copied()
    }
}

/// The pixel type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pixel {
    Float,
    Double,
    Complex,
    DComplex,
}

impl Pixel {
    pub fn is_complex(self) -> bool {
        matches!(self, Pixel::Complex | Pixel::DComplex)
    }

    pub fn name(self) -> &'static str {
        match self {
            Pixel::Float => "Float",
            Pixel::Double => "Double",
            Pixel::Complex => "Complex",
            Pixel::DComplex => "DComplex",
        }
    }

    pub fn bits(self) -> u8 {
        match self {
            Pixel::Float => 32,
            Pixel::Double | Pixel::Complex => 64,
            Pixel::DComplex => 128,
        }
    }
}

/// Where a column's cell lives.
#[derive(Clone, Debug)]
struct Cell {
    tsm: tsm::Tsm,
    cube: tsm::Cube,
    /// Position along the cube's row axis (TiledShapeStMan).
    row: Option<usize>,
}

impl Cell {
    fn open(dir: &Path, t: &Table, column: &str) -> Result<Cell> {
        let col = t
            .column(column)
            .ok_or_else(|| Error::Malformed(format!("no column {column}")))?;
        if !col.array {
            return Err(Error::Malformed(format!("column {column} is not an array")));
        }
        if t.rows == 0 {
            return Err(Error::Malformed("the image table has no rows".into()));
        }
        let kind = t.manager(col.manager).unwrap_or("");
        if !matches!(kind, "TiledCellStMan" | "TiledShapeStMan") {
            return Err(Error::Unsupported(format!("pixels stored by {kind}")));
        }
        let tsm = tsm::Tsm::open(dir, col.manager)?;
        if tsm.dtype != col.dtype {
            return Err(Error::Malformed("column and storage types differ".into()));
        }
        let (cube, row) = tsm.locate(0)?;
        let cube = cube.clone();
        tsm.check(&cube)?;
        Ok(Cell { tsm, cube, row })
    }

    /// The cell's shape (the cube without its row axis).
    fn shape(&self) -> &[usize] {
        let s = &self.cube.shape;
        if self.row.is_some() {
            &s[..s.len().saturating_sub(1)]
        } else {
            s
        }
    }

    /// The elements of the box `start .. start + count` of the cell.
    fn region<T: Copy>(
        &self,
        start: &[usize],
        count: &[usize],
        fill: T,
        dec: impl Fn(&[u8], usize) -> T,
    ) -> Result<Vec<T>> {
        let (mut s, mut c) = (start.to_vec(), count.to_vec());
        if let Some(r) = self.row {
            s.push(r);
            c.push(1);
        }
        self.tsm.region(&self.cube, &s, &c, fill, dec)
    }
}

/// An opened CASA image.
#[derive(Clone, Debug)]
pub struct CasaImage {
    pub dir: PathBuf,
    /// Axis lengths, first axis fastest (usually RA, Dec, Stokes, frequency).
    pub shape: Vec<usize>,
    pub pixel: Pixel,
    /// The table keywords (`coords`, `imageinfo`, `units`, `miscinfo`, `masks`...).
    pub keywords: Record,
    pub coords: Coords,
    /// Brightness unit (`Jy/beam`).
    pub units: Option<String>,
    pub object: Option<String>,
    /// `imageinfo.imagetype` (`Intensity`, `Beam`...).
    pub info_type: Option<String>,
    /// From the directory name.
    pub image_type: ImageType,
    pub beam: Option<Beam>,
    pub plane_beams: Option<PlaneBeams>,
    /// The default mask's name, when it was read.
    pub mask_name: Option<String>,
    /// Data are big-endian.
    pub big_endian: bool,
    /// The storage manager (`TiledCellStMan`, `TiledShapeStMan`).
    pub storage: &'static str,
    /// The tile shape.
    pub tile: Vec<usize>,
    cell: Cell,
    mask: Option<Cell>,
}

/// A plane's values, first axis fastest.
#[derive(Clone, Debug, PartialEq)]
pub enum PlaneData {
    Real(Vec<f32>),
    /// Real and imaginary parts.
    Complex(Vec<[f32; 2]>),
}

impl CasaImage {
    /// Opens the image at `path`: its directory, or `table.dat` / `table.f0` in it.
    pub fn open(path: &Path) -> Result<CasaImage> {
        let dir = image_dir(path);
        let data =
            read_capped(&dir.join("table.dat"), table::MAX_TABLE_DAT).map_err(|e| match e {
                Error::Io(_) => Error::NotCasa,
                e => e,
            })?;
        if !table::sniff(&data) {
            return Err(Error::NotCasa);
        }
        let t = Table::parse(&data)?;
        let Some(coords_rec) = t.keywords.record("coords") else {
            return Err(Error::NotCasa);
        };
        let column = if t.column("map").is_some() {
            "map"
        } else {
            t.columns
                .iter()
                .find(|c| c.array)
                .map(|c| c.name.as_str())
                .ok_or_else(|| Error::Malformed("no array column".into()))?
        };
        let cell = Cell::open(&dir, &t, column)?;
        let pixel = match cell.tsm.dtype {
            dtype::FLOAT => Pixel::Float,
            dtype::DOUBLE => Pixel::Double,
            dtype::COMPLEX => Pixel::Complex,
            dtype::DCOMPLEX => Pixel::DComplex,
            d => {
                return Err(Error::Unsupported(format!("{} pixels", dtype::name(d))));
            }
        };
        let shape = cell.shape().to_vec();
        if shape.is_empty() || shape.contains(&0) {
            return Err(Error::Malformed("empty image".into()));
        }
        let coords = coords::parse(coords_rec, shape.len());
        let info = t.keywords.record("imageinfo");
        let beam = info
            .and_then(|i| i.record("restoringbeam"))
            .and_then(Beam::from_record);
        let plane_beams = info
            .and_then(|i| i.record("perplanebeams"))
            .and_then(plane_beams);
        let units = t
            .keywords
            .str("units")
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty());
        let object = info
            .and_then(|i| i.str("objectname"))
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty());
        let info_type = info
            .and_then(|i| i.str("imagetype"))
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty());
        let (mask_name, mask) = match default_mask(&dir, &t, &shape) {
            Some((n, m)) => (Some(n), Some(m)),
            None => (None, None),
        };
        let storage = match cell.tsm.kind {
            tsm::Kind::Cell => "TiledCellStMan",
            tsm::Kind::Shape => "TiledShapeStMan",
        };
        let tile = cell.cube.tile[..shape.len()].to_vec();
        Ok(CasaImage {
            image_type: ImageType::from_name(&dir),
            dir,
            shape,
            pixel,
            keywords: t.keywords.clone(),
            coords,
            units,
            object,
            info_type,
            beam,
            plane_beams,
            mask_name,
            big_endian: cell.tsm.big_endian,
            storage,
            tile,
            cell,
            mask,
        })
    }

    pub fn width(&self) -> usize {
        self.shape[0]
    }

    pub fn height(&self) -> usize {
        self.shape.get(1).copied().unwrap_or(1)
    }

    /// Planes along the axes after the first two.
    pub fn planes(&self) -> usize {
        self.shape.iter().skip(2).product()
    }

    pub fn has_mask(&self) -> bool {
        self.mask.is_some()
    }

    /// The beam of the plane at `index` (positions on the axes after the first two).
    pub fn beam_at(&self, index: &[usize]) -> Option<Beam> {
        if let Some(pb) = &self.plane_beams {
            let at = |kind: AxisKind| {
                self.coords
                    .axes
                    .iter()
                    .position(|a| a.kind == kind)
                    .filter(|&k| k >= 2)
                    .and_then(|k| index.get(k - 2).copied())
                    .unwrap_or(0)
            };
            return pb.get(at(AxisKind::Spectral), at(AxisKind::Stokes));
        }
        self.beam
    }

    fn plane_box(&self, index: &[usize]) -> Result<(Vec<usize>, Vec<usize>)> {
        let nd = self.shape.len();
        if index.len() != nd.saturating_sub(2)
            || index
                .iter()
                .zip(&self.shape[2.min(nd)..])
                .any(|(&i, &n)| i >= n)
        {
            return Err(Error::Malformed("no such plane".into()));
        }
        let mut start = vec![0; nd];
        let mut count = vec![1; nd];
        count[0] = self.shape[0];
        if nd > 1 {
            count[1] = self.shape[1];
        }
        start[2.min(nd)..].copy_from_slice(index);
        Ok((start, count))
    }

    /// The plane at `index` (positions on the axes after the first two), first axis fastest,
    /// masked pixels NaN.
    pub fn plane(&self, index: &[usize]) -> Result<PlaneData> {
        let (start, count) = self.plane_box(index)?;
        let mut data = self.read(&start, &count)?;
        // The plane is counted while its mask is read beside it.
        let _held = specular_core::budget::reserve(match &data {
            PlaneData::Real(v) => size_of_val(v.as_slice()),
            PlaneData::Complex(v) => size_of_val(v.as_slice()),
        } as u64)?;
        if let Some(m) = self.mask_region(&start, &count)? {
            match &mut data {
                PlaneData::Real(v) => {
                    for (x, ok) in v.iter_mut().zip(m) {
                        if !ok {
                            *x = f32::NAN;
                        }
                    }
                }
                PlaneData::Complex(v) => {
                    for (x, ok) in v.iter_mut().zip(m) {
                        if !ok {
                            *x = [f32::NAN; 2];
                        }
                    }
                }
            }
        }
        Ok(data)
    }

    /// The default mask of the plane at `index` (true where valid); `None` without a mask.
    pub fn plane_mask(&self, index: &[usize]) -> Result<Option<Vec<bool>>> {
        let (start, count) = self.plane_box(index)?;
        self.mask_region(&start, &count)
    }

    /// The values along `axis` through pixel `at` (a position on every axis; its entry on
    /// `axis` is ignored), masked values NaN; complex images give amplitudes.
    pub fn line(&self, at: &[usize], axis: usize) -> Result<Vec<f32>> {
        let nd = self.shape.len();
        if at.len() != nd || axis >= nd || (0..nd).any(|i| i != axis && at[i] >= self.shape[i]) {
            return Err(Error::Malformed("no such pixel".into()));
        }
        let mut start = at.to_vec();
        start[axis] = 0;
        let mut count = vec![1; nd];
        count[axis] = self.shape[axis];
        let mut v = match self.read(&start, &count)? {
            PlaneData::Real(v) => v,
            PlaneData::Complex(v) => v.iter().map(|c| c[0].hypot(c[1])).collect(),
        };
        let _held = specular_core::budget::reserve(size_of_val(v.as_slice()) as u64)?;
        if let Some(m) = self.mask_region(&start, &count)? {
            for (x, ok) in v.iter_mut().zip(m) {
                if !ok {
                    *x = f32::NAN;
                }
            }
        }
        Ok(v)
    }

    /// The box `start .. start + count`, unmasked.
    pub fn read(&self, start: &[usize], count: &[usize]) -> Result<PlaneData> {
        let be = self.cell.tsm.big_endian;
        let f32_at = move |b: &[u8], o: usize| {
            let a = [b[o], b[o + 1], b[o + 2], b[o + 3]];
            if be {
                f32::from_be_bytes(a)
            } else {
                f32::from_le_bytes(a)
            }
        };
        let f64_at = move |b: &[u8], o: usize| {
            let mut a = [0; 8];
            a.copy_from_slice(&b[o..o + 8]);
            if be {
                f64::from_be_bytes(a)
            } else {
                f64::from_le_bytes(a)
            }
        };
        Ok(match self.pixel {
            Pixel::Float => PlaneData::Real(
                self.cell
                    .region(start, count, 0.0, |b, k| f32_at(b, k * 4))?,
            ),
            Pixel::Double => PlaneData::Real(
                self.cell
                    .region(start, count, 0.0, |b, k| f64_at(b, k * 8) as f32)?,
            ),
            Pixel::Complex => {
                PlaneData::Complex(self.cell.region(start, count, [0.0; 2], |b, k| {
                    [f32_at(b, k * 8), f32_at(b, k * 8 + 4)]
                })?)
            }
            Pixel::DComplex => {
                PlaneData::Complex(self.cell.region(start, count, [0.0; 2], |b, k| {
                    [f64_at(b, k * 16) as f32, f64_at(b, k * 16 + 8) as f32]
                })?)
            }
        })
    }

    fn mask_region(&self, start: &[usize], count: &[usize]) -> Result<Option<Vec<bool>>> {
        let Some(m) = &self.mask else {
            return Ok(None);
        };
        m.region(start, count, true, |b, k| b[k / 8] >> (k % 8) & 1 != 0)
            .map(Some)
    }

    /// FITS cards for the image: the WCS, and `BUNIT`, `OBJECT`, `TELESCOP`, `OBSERVER`,
    /// `DATE-OBS`, `BMAJ`/`BMIN`/`BPA` (degrees).
    pub fn fits_cards(&self) -> Vec<(String, String)> {
        use coords::{fits_num, fits_str};
        let mut c = self.coords.cards.clone();
        for (k, n) in self.shape.iter().enumerate() {
            c.push((format!("NAXIS{}", k + 1), n.to_string()));
        }
        if let Some(u) = &self.units {
            c.push(("BUNIT".into(), fits_str(u)));
        }
        if let Some(o) = &self.object {
            c.push(("OBJECT".into(), fits_str(o)));
        }
        if let Some(t) = &self.coords.telescope {
            c.push(("TELESCOP".into(), fits_str(t)));
        }
        if let Some(o) = &self.coords.observer {
            c.push(("OBSERVER".into(), fits_str(o)));
        }
        if let Some(d) = &self.coords.date {
            c.push(("DATE-OBS".into(), fits_str(d)));
        }
        if let Some(b) = self.beam {
            c.push(("BMAJ".into(), fits_num(b.major / 3600.0)));
            c.push(("BMIN".into(), fits_num(b.minor / 3600.0)));
            c.push(("BPA".into(), fits_num(b.pa)));
        }
        c
    }
}

fn plane_beams(r: &Record) -> Option<PlaneBeams> {
    let channels = usize::try_from(r.f64("nChannels")? as i64).ok()?;
    let stokes = usize::try_from(r.f64("nStokes")? as i64).ok()?;
    let n = channels.checked_mul(stokes)?;
    if n == 0 || n > 1 << 24 {
        return None;
    }
    let mut beams = Vec::with_capacity(n.min(r.fields.len()));
    for k in 0..n {
        beams.push(Beam::from_record(r.record(&format!("*{k}"))?)?);
    }
    Some(PlaneBeams {
        channels,
        stokes,
        beams,
    })
}

/// The default mask: its name and cell, when it can be read and fits the image.
fn default_mask(dir: &Path, t: &Table, shape: &[usize]) -> Option<(String, Cell)> {
    let name = t.keywords.str("Image_defaultmask")?.trim().to_owned();
    if name.is_empty() {
        return None;
    }
    let entry = t.keywords.record("masks")?.record(&name)?;
    let table = entry.str("mask")?;
    // Subtables are named relative to the image (`././mask0`, `Table: /abs/x.image/mask0`).
    let leaf = table
        .rsplit('/')
        .next()
        .map(str::trim)
        .filter(|l| !l.is_empty() && *l != "." && *l != "..")?;
    let mdir = dir.join(leaf);
    let data = read_capped(&mdir.join("table.dat"), table::MAX_TABLE_DAT).ok()?;
    let mt = Table::parse(&data).ok()?;
    let column = if mt.column("PagedArray").is_some() {
        "PagedArray"
    } else {
        mt.columns.iter().find(|c| c.array)?.name.as_str()
    };
    let cell = Cell::open(&mdir, &mt, column).ok()?;
    (cell.tsm.dtype == dtype::BOOL && cell.shape() == shape).then_some((name, cell))
}

#[cfg(test)]
mod tests;
