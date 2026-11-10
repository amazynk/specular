//! VTK files: own readers for the legacy format (`.vtk`, versions 1.0 to 5.1, ASCII and
//! big-endian binary) and the XML formats (`.vti`, `.vtr`, `.vts`, `.vtu`, `.vtp`).
//!
//! * **Grids** (structured points / image data, rectilinear and structured grids) come back
//!   as a [`Grid`]: the point counts along x, y and z, the geometry, and every point and cell
//!   array as `f32` values, x fastest.
//! * **Polygonal data and unstructured grids** come back as a [`MeshData`]: the points and
//!   the triangles a viewer draws. Polygons are fan-triangulated, strips split, 3D cells
//!   reduced to their boundary faces (faces no other cell shares); vertices and lines are
//!   counted and skipped. Point and cell arrays are kept, with each triangle's cell.
//! * **XML encodings:** `ascii`, inline `binary` (base64) and `appended` (raw or base64),
//!   UInt32 or UInt64 block headers, either byte order, uncompressed, zlib or LZ4 blocks.
//!   LZMA blocks and parallel (`.pvti`, `.pvtu`…) files are refused with a reason; string
//!   arrays are skipped in ASCII files and XML field data, refused elsewhere.
//!
//! Every count is checked against the bytes present before anything is allocated, and no
//! input panics.

mod base64;
mod cells;
mod legacy;
mod num;
mod xml;

#[cfg(test)]
mod tests;

/// What a file holds, from its first bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A legacy `.vtk` file (any dataset type).
    Legacy,
    ImageData,
    RectilinearGrid,
    StructuredGrid,
    UnstructuredGrid,
    PolyData,
}

impl Kind {
    /// The file type as VTK names it ("legacy VTK", "ImageData"…).
    pub fn name(self) -> &'static str {
        match self {
            Kind::Legacy => "legacy VTK",
            Kind::ImageData => "ImageData",
            Kind::RectilinearGrid => "RectilinearGrid",
            Kind::StructuredGrid => "StructuredGrid",
            Kind::UnstructuredGrid => "UnstructuredGrid",
            Kind::PolyData => "PolyData",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not a VTK file")]
    NotVtk,
    #[error("truncated VTK file: {0}")]
    Truncated(String),
    #[error("malformed VTK file: {0}")]
    Malformed(String),
    #[error("{0}")]
    Unsupported(String),
    #[error("VTK data too large")]
    TooLarge,
}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn bad<T>(msg: impl Into<String>) -> Result<T> {
    Err(Error::Malformed(msg.into()))
}

/// Caps on what one file may make us allocate.
pub(crate) mod limits {
    /// Values (all components) of one array, points of a mesh, cells.
    pub const MAX_VALUES: u64 = 1 << 32;
    /// Decompressed bytes of one array.
    pub const MAX_BYTES: u64 = 16 << 30;
    /// Arrays per section.
    pub const MAX_ARRAYS: usize = 4096;
    /// Pieces of an XML file.
    pub const MAX_PIECES: usize = 65_536;
}

/// A parsed file.
#[derive(Clone, Debug)]
pub struct Vtk {
    pub kind: Kind,
    /// The legacy header's title line (XML files have none).
    pub title: Option<String>,
    /// The file format version ("4.2", "5.1"; XML: the VTKFile version, "0.1", "2.2").
    pub version: String,
    /// How the data is stored: "ASCII", "binary", "ascii", "base64", "appended raw",
    /// "appended base64", with the compressor (", zlib", ", LZ4") and, for XML binary data,
    /// the byte order when big-endian.
    pub encoding: String,
    pub dataset: Dataset,
    /// Arrays of the dataset itself (legacy `FIELD`, XML `FieldData`: a time value…).
    pub field_data: Vec<DataArray>,
}

#[derive(Clone, Debug)]
pub enum Dataset {
    Grid(Grid),
    Mesh(MeshData),
}

/// A structured dataset: values on a regular lattice of points (and of cells between them).
#[derive(Clone, Debug, Default)]
pub struct Grid {
    /// "STRUCTURED_POINTS", "RECTILINEAR_GRID", "STRUCTURED_GRID" (legacy) or "ImageData",
    /// "RectilinearGrid", "StructuredGrid" (XML).
    pub structure: &'static str,
    /// Points along x, y and z (1 for a missing dimension).
    pub dims: [usize; 3],
    /// Image data: the position of the first point and the step along each axis.
    pub origin: [f64; 3],
    pub spacing: [f64; 3],
    /// Image data: the axes' directions (row-major 3×3), when the file gives them.
    pub direction: Option<[f64; 9]>,
    /// Rectilinear grids: the coordinates along x, y and z.
    pub coords: Option<[Vec<f64>; 3]>,
    /// Structured grids: every point's position, x fastest.
    pub points: Option<Vec<[f32; 3]>>,
    /// Arrays on the points: `dims[0] × dims[1] × dims[2]` tuples, x fastest.
    pub point_data: Vec<DataArray>,
    /// Arrays on the cells: `max(dims[i] − 1, 1)` along each axis, x fastest.
    pub cell_data: Vec<DataArray>,
}

impl Grid {
    /// Cells along x, y and z.
    pub fn cell_dims(&self) -> [usize; 3] {
        self.dims.map(|d| d.saturating_sub(1).max(1))
    }
}

/// One named array.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DataArray {
    pub name: String,
    pub components: usize,
    /// The element type as the file names it ("float", "unsigned_char", "Float32", "Int64").
    pub dtype: &'static str,
    /// Interleaved components, tuple after tuple.
    pub values: Vec<f32>,
    /// The attribute role the file gives the array: legacy "SCALARS", "COLOR_SCALARS",
    /// "VECTORS", "NORMALS", "TEXTURE_COORDINATES", "TENSORS", "GLOBAL_IDS",
    /// "PEDIGREE_IDS", "EDGE_FLAGS", "FIELD"; XML "Scalars", "Vectors", "Normals",
    /// "Tensors", "TCoords" (the section's active arrays). `None` for other XML arrays.
    pub attribute: Option<&'static str>,
}

impl DataArray {
    /// Number of tuples.
    pub fn tuples(&self) -> usize {
        self.values.len() / self.components.max(1)
    }
}

/// Polygonal data or an unstructured grid, as the triangles of its surface.
#[derive(Clone, Debug, Default)]
pub struct MeshData {
    pub points: Vec<[f32; 3]>,
    /// Indices into `points`.
    pub triangles: Vec<[u32; 3]>,
    /// One tuple per point.
    pub point_data: Vec<DataArray>,
    /// One tuple per cell of the file (polygonal data: vertices, lines, polygons, then
    /// strips, VTK's order).
    pub cell_data: Vec<DataArray>,
    /// The cell (index into the cell arrays) of each triangle.
    pub triangle_cell: Vec<u32>,
    /// Cells in the file, of every kind.
    pub cells: u64,
    /// Vertex and line cells (not drawn).
    pub vertices: u64,
    pub lines: u64,
    /// What was skipped (unknown cell types, invalid cells).
    pub notes: Vec<String>,
}

/// What the first bytes say: a legacy file or the XML file type.
pub fn sniff(head: &[u8]) -> Option<Kind> {
    if head.starts_with(b"# vtk DataFile Version") || head.starts_with(b"# vtk DataFile version") {
        return Some(Kind::Legacy);
    }
    let h = head.strip_prefix(b"\xef\xbb\xbf").unwrap_or(head);
    let at = find(h, b"<VTKFile")?;
    // Only an XML declaration, comments and white space before the root element.
    let before = &h[..at];
    let trimmed: Vec<u8> = before
        .iter()
        .copied()
        .filter(|b| !b.is_ascii_whitespace())
        .collect();
    if !(trimmed.is_empty() || trimmed.starts_with(b"<?xml") || trimmed.starts_with(b"<!--")) {
        return None;
    }
    let tag_end = h[at..]
        .iter()
        .position(|&b| b == b'>')
        .map_or(h.len(), |e| at + e);
    let tag = &h[at..tag_end];
    let ty = xml::attr_in_tag(tag, b"type")?;
    match ty.as_slice() {
        b"ImageData" => Some(Kind::ImageData),
        b"RectilinearGrid" => Some(Kind::RectilinearGrid),
        b"StructuredGrid" => Some(Kind::StructuredGrid),
        b"UnstructuredGrid" => Some(Kind::UnstructuredGrid),
        b"PolyData" => Some(Kind::PolyData),
        _ => None,
    }
}

/// Whether the first bytes are a VTK XML file of a type this crate refuses (parallel files,
/// multiblock sets, hyper tree grids): the decoder can still name it and give the reason.
pub fn sniff_other_xml(head: &[u8]) -> bool {
    let h = head.strip_prefix(b"\xef\xbb\xbf").unwrap_or(head);
    find(h, b"<VTKFile").is_some_and(|at| at < 512) && sniff(head).is_none()
}

pub(crate) fn find(h: &[u8], needle: &[u8]) -> Option<usize> {
    h.windows(needle.len()).position(|w| w == needle)
}

/// Reads a whole file.
pub fn parse(data: &[u8]) -> Result<Vtk> {
    let head = &data[..data.len().min(4096)];
    if head.starts_with(b"# vtk DataFile") {
        return legacy::parse(data);
    }
    let h = head.strip_prefix(b"\xef\xbb\xbf").unwrap_or(head);
    if find(h, b"<VTKFile").is_some() {
        return xml::parse(data);
    }
    Err(Error::NotVtk)
}
