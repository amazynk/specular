//! Unstructured meshes stored in NetCDF: UGRID 2-D mesh topologies (a variable with
//! `cf_role = "mesh_topology"` and `topology_dimension = 2`, as FVCOM, ADCIRC, SCHISM,
//! Delft3D-FM and MPAS-style converters write them) and ICON grids (cell centres `clon` /
//! `clat` with vertex bounds, or `vertex_of_cell` over `vlon` / `vlat`, in radians).
//!
//! [`detect`] finds the topology, [`Mesh::load`] reads node coordinates and the face–node
//! connectivity (either layout, `start_index` 0 or 1, fill values for mixed triangles,
//! quads and polygons) with every count capped against the file and every index checked,
//! [`variables`] lists the data variables on the faces and nodes, and [`read_values`]
//! reads one field. [`Display`] fan-triangulates the faces for drawing (longitudes in one
//! window, faces across the antimeridian split into copies, polar faces closed to the
//! pole), [`Locator`] finds the face under a point through a uniform grid, and [`raster`]
//! paints a field onto a pixel grid.

mod display;
mod index;
mod raster;

pub use display::Display;
pub use index::{Hit, Locator};
pub use raster::{Frame, raster, segments_in};

use specular_netcdf::{Netcdf, Value, Variable};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no unstructured mesh")]
    NotUgrid,
    #[error("malformed mesh: {0}")]
    Malformed(String),
    #[error("mesh too large")]
    TooLarge,
    #[error("cancelled")]
    Cancelled,
    #[error(transparent)]
    Netcdf(#[from] specular_netcdf::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Caps against hostile or broken files.
pub mod limits {
    /// Nodes of a mesh.
    pub const MAX_NODES: u64 = 1 << 25;
    /// Faces of a mesh.
    pub const MAX_FACES: u64 = 1 << 25;
    /// Nodes of one face.
    pub const MAX_FACE_NODES: u64 = 64;
    /// Elements of the face–node connectivity.
    pub const MAX_CONNECTIVITY: u64 = 1 << 27;
    /// How many values a file can hold per byte: deflate's best ratio, so compressed
    /// NetCDF-4 variables pass and a header that claims more than the file can hold
    /// does not.
    pub const VALUES_PER_BYTE: u64 = 1100;
    /// Faces read per block.
    pub(crate) const BLOCK: u64 = 1 << 18;
}

/// Where a data variable's values sit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Location {
    /// One value per face, drawn flat.
    Face,
    /// One value per node, interpolated across the faces.
    Node,
}

/// What the node coordinates are.
#[derive(Clone, Debug, PartialEq)]
pub enum Coords {
    /// Longitude and latitude, degrees (radians in the file are converted).
    LonLat,
    /// Projected or Cartesian x and y, in `unit` when the file says.
    Projected { unit: Option<String> },
}

/// Which convention describes the mesh.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Convention {
    Ugrid,
    Icon,
    /// MPAS native: `verticesOnCell` over `lonVertex` / `latVertex` (radians), 1-based
    /// with 0 past a cell's last vertex.
    Mpas,
}

/// Where the faces' nodes come from.
#[derive(Clone, Debug)]
enum Faces {
    /// A face–node connectivity variable; `transposed` when stored nodes-by-faces (FVCOM's
    /// `nv`, ICON's `vertex_of_cell`).
    Connectivity {
        var: usize,
        transposed: bool,
        start: Option<i64>,
    },
    /// Per-face vertex longitudes and latitudes (ICON's `clon_vertices` / `clon_bnds`); the
    /// nodes are the distinct vertices.
    Bounds { lon: usize, lat: usize },
}

/// A mesh topology found in a file.
#[derive(Clone, Debug)]
pub struct Topology {
    /// The topology variable's name, or the convention's ("ICON grid").
    pub name: String,
    pub long_name: Option<String>,
    pub convention: Convention,
    pub coords: Coords,
    /// The dimension the faces run along.
    pub face_dim: String,
    /// The dimension the nodes run along (`None` when the nodes are made from per-face
    /// vertices).
    pub node_dim: Option<String>,
    pub faces: u64,
    /// Edges, when the file stores an edge–node connectivity.
    pub edges: Option<u64>,
    node_x: Option<usize>,
    node_y: Option<usize>,
    radians: bool,
    source: Faces,
    /// Variables that describe the mesh rather than hold data on it.
    structural: Vec<usize>,
}

fn text<'a>(v: &'a Variable, name: &str) -> Option<&'a str> {
    v.text(name)
}

fn num(v: &Variable, name: &str) -> Option<f64> {
    v.attr(name).and_then(Value::as_f64)
}

fn var_index(nc: &Netcdf, name: &str) -> Option<usize> {
    nc.vars.iter().position(|v| v.name == name)
}

/// The role of a coordinate variable: Some(true) for x / longitude, Some(false) for y /
/// latitude.
fn axis_role(v: &Variable) -> Option<bool> {
    let sn = text(v, "standard_name").unwrap_or("").to_ascii_lowercase();
    if sn.contains("longitude") || sn.contains("x_coordinate") {
        return Some(true);
    }
    if sn.contains("latitude") || sn.contains("y_coordinate") {
        return Some(false);
    }
    let units = text(v, "units").unwrap_or("").to_ascii_lowercase();
    if units.contains("east") || units == "degree_e" || units == "degrees_e" {
        return Some(true);
    }
    if units.contains("north") || units == "degree_n" || units == "degrees_n" {
        return Some(false);
    }
    match text(v, "axis").map(str::to_ascii_uppercase).as_deref() {
        Some("X") => return Some(true),
        Some("Y") => return Some(false),
        _ => {}
    }
    let n = v
        .name
        .rsplit('/')
        .next()
        .unwrap_or(&v.name)
        .to_ascii_lowercase();
    if n.contains("lon") || n.ends_with('x') {
        Some(true)
    } else if n.contains("lat") || n.ends_with('y') {
        Some(false)
    } else {
        None
    }
}

/// Whether coordinates are geographic, and whether in radians.
fn geographic(x: &Variable, y: &Variable) -> (bool, bool) {
    let units = |v: &Variable| text(v, "units").unwrap_or("").to_ascii_lowercase();
    let sn = |v: &Variable| text(v, "standard_name").unwrap_or("").to_ascii_lowercase();
    let (ux, uy) = (units(x), units(y));
    let radians = ux.starts_with("radian") && uy.starts_with("radian");
    let degrees = ux.starts_with("degree") || uy.starts_with("degree");
    let names = sn(x).contains("longitude") || sn(y).contains("latitude");
    let projected = sn(x).contains("projection") || ux == "m" || ux.starts_with("met");
    (!projected && (radians || degrees || names), radians)
}

/// Orders two coordinate variables as (x, y).
fn order_xy(nc: &Netcdf, a: usize, b: usize) -> (usize, usize) {
    match (axis_role(&nc.vars[a]), axis_role(&nc.vars[b])) {
        (Some(false), _) | (_, Some(true)) => (b, a),
        _ => (a, b),
    }
}

/// Finds the 2-D mesh topology of a NetCDF file: the first UGRID `mesh_topology` variable
/// with `topology_dimension = 2`, else an ICON grid.
pub fn detect(nc: &Netcdf) -> Option<Topology> {
    for (i, v) in nc.vars.iter().enumerate() {
        if text(v, "cf_role") == Some("mesh_topology")
            && num(v, "topology_dimension") == Some(2.0)
            && let Some(t) = ugrid(nc, i)
        {
            return Some(t);
        }
    }
    icon(nc).or_else(|| mpas(nc))
}

/// MPAS mesh variables other than the vertices and connectivity: not data pages.
const MPAS_STRUCTURAL: &[&str] = &[
    "lonCell",
    "latCell",
    "xCell",
    "yCell",
    "zCell",
    "lonEdge",
    "latEdge",
    "xEdge",
    "yEdge",
    "zEdge",
    "lonVertex",
    "latVertex",
    "xVertex",
    "yVertex",
    "zVertex",
    "indexToCellID",
    "indexToEdgeID",
    "indexToVertexID",
    "cellsOnCell",
    "edgesOnCell",
    "verticesOnCell",
    "cellsOnEdge",
    "verticesOnEdge",
    "edgesOnEdge",
    "cellsOnVertex",
    "edgesOnVertex",
    "nEdgesOnCell",
    "nEdgesOnEdge",
    "weightsOnEdge",
    "kiteAreasOnVertex",
    "edgeSignOnCell",
    "edgeSignOnVertex",
    "kiteForCell",
    "cellQuality",
    "gridSpacing",
    "triangleQuality",
    "triangleAngleQuality",
    "obtuseTriangle",
    "boundaryVertex",
    "meshDensity",
];

fn mpas(nc: &Netcdf) -> Option<Topology> {
    let voc = var_index(nc, "verticesOnCell")?;
    let c = &nc.vars[voc];
    if c.shape.len() != 2 || c.num.is_none() || !(3..=limits::MAX_FACE_NODES).contains(&c.shape[1])
    {
        return None;
    }
    let sphere = !nc
        .attr("on_a_sphere")
        .and_then(Value::as_str)
        .is_some_and(|s| s.trim().eq_ignore_ascii_case("NO"));
    let (xs, ys) = if sphere {
        ("lonVertex", "latVertex")
    } else {
        ("xVertex", "yVertex")
    };
    let (nx, ny) = (var_index(nc, xs)?, var_index(nc, ys)?);
    let vx = &nc.vars[nx];
    if vx.shape.len() != 1 || nc.vars[ny].shape != vx.shape {
        return None;
    }
    let structural = MPAS_STRUCTURAL
        .iter()
        .filter_map(|n| var_index(nc, n))
        .collect();
    let edges = var_index(nc, "verticesOnEdge").and_then(|e| nc.vars[e].shape.first().copied());
    let degrees = text(vx, "units").is_some_and(|u| u.to_ascii_lowercase().starts_with("degree"));
    Some(Topology {
        name: "MPAS mesh".into(),
        long_name: None,
        convention: Convention::Mpas,
        coords: if sphere {
            Coords::LonLat
        } else {
            Coords::Projected {
                unit: text(vx, "units").map(str::to_owned),
            }
        },
        face_dim: c.dims[0].clone(),
        node_dim: Some(vx.dims[0].clone()),
        faces: c.shape[0],
        edges,
        node_x: Some(nx),
        node_y: Some(ny),
        // MPAS stores spherical positions in radians whether or not `units` says so.
        radians: sphere && !degrees,
        source: Faces::Connectivity {
            var: voc,
            transposed: false,
            start: Some(1),
        },
        structural,
    })
}

fn ugrid(nc: &Netcdf, mesh: usize) -> Option<Topology> {
    let m = &nc.vars[mesh];
    let names = |attr: &str| -> Vec<usize> {
        text(m, attr)
            .map(|s| {
                s.split_whitespace()
                    .filter_map(|n| var_index(nc, n))
                    .collect()
            })
            .unwrap_or_default()
    };
    let coords = names("node_coordinates");
    if coords.len() < 2 {
        return None;
    }
    let (nx, ny) = order_xy(nc, coords[0], coords[1]);
    let node_dim = nc.vars[nx].dims.first()?.clone();
    if nc.vars[nx].shape.len() != 1 || nc.vars[ny].shape != nc.vars[nx].shape {
        return None;
    }
    let conn = *names("face_node_connectivity").first()?;
    let c = &nc.vars[conn];
    if c.shape.len() != 2 || c.num.is_none() {
        return None;
    }
    // Which dimension runs along the faces: `face_dimension`, else the long one.
    let transposed = match text(m, "face_dimension") {
        Some(fd) if c.dims[1] == fd && c.dims[0] != fd => true,
        Some(fd) if c.dims[0] == fd => false,
        _ => c.shape[0] < c.shape[1] && c.shape[0] <= limits::MAX_FACE_NODES,
    };
    let (fdim, nmax) = if transposed { (1, 0) } else { (0, 1) };
    if c.shape[nmax] > limits::MAX_FACE_NODES || c.shape[nmax] < 3 {
        return None;
    }
    let (geo, radians) = geographic(&nc.vars[nx], &nc.vars[ny]);
    let start = num(c, "start_index").map(|s| s as i64);
    let mut structural = vec![mesh, nx, ny, conn];
    for attr in [
        "face_coordinates",
        "edge_coordinates",
        "edge_node_connectivity",
        "face_edge_connectivity",
        "face_face_connectivity",
        "edge_face_connectivity",
        "boundary_node_connectivity",
    ] {
        structural.extend(names(attr));
    }
    // Bounds of the coordinates (face node coordinates).
    for &k in &structural.clone() {
        if let Some(b) = text(&nc.vars[k], "bounds").and_then(|b| var_index(nc, b)) {
            structural.push(b);
        }
    }
    let edges = names("edge_node_connectivity").first().and_then(|&e| {
        let s = &nc.vars[e].shape;
        (s.len() == 2).then(|| if s[0] == 2 && s[1] != 2 { s[1] } else { s[0] })
    });
    Some(Topology {
        name: m.name.clone(),
        long_name: text(m, "long_name").map(str::to_owned),
        convention: Convention::Ugrid,
        coords: if geo {
            Coords::LonLat
        } else {
            Coords::Projected {
                unit: text(&nc.vars[nx], "units").map(str::to_owned),
            }
        },
        face_dim: c.dims[fdim].clone(),
        node_dim: Some(node_dim),
        faces: c.shape[fdim],
        edges,
        node_x: Some(nx),
        node_y: Some(ny),
        radians,
        source: Faces::Connectivity {
            var: conn,
            transposed,
            start,
        },
        structural,
    })
}

fn icon(nc: &Netcdf) -> Option<Topology> {
    let clon = var_index(nc, "clon")?;
    let clat = var_index(nc, "clat")?;
    let (lo, la) = (&nc.vars[clon], &nc.vars[clat]);
    if lo.shape.len() != 1 || la.shape != lo.shape {
        return None;
    }
    let cell_dim = lo.dims[0].clone();
    let cells = lo.shape[0];
    let radians = !text(lo, "units").is_some_and(|u| u.to_ascii_lowercase().starts_with("degree"));
    let mut structural = vec![clon, clat];
    // An ICON grid file: vertex_of_cell (nv, cell), 1-based, over vlon / vlat.
    if let (Some(voc), Some(vlon), Some(vlat)) = (
        var_index(nc, "vertex_of_cell"),
        var_index(nc, "vlon"),
        var_index(nc, "vlat"),
    ) {
        let c = &nc.vars[voc];
        let vl = &nc.vars[vlon];
        if c.shape.len() == 2
            && c.num.is_some()
            && vl.shape.len() == 1
            && nc.vars[vlat].shape == vl.shape
            && (c.shape[1] == cells || c.shape[0] == cells)
        {
            let transposed = c.shape[1] == cells && c.dims[1] == cell_dim;
            let nmax = if transposed { c.shape[0] } else { c.shape[1] };
            if (3..=limits::MAX_FACE_NODES).contains(&nmax) {
                structural.extend([voc, vlon, vlat]);
                for k in [clon, clat, vlon, vlat] {
                    if let Some(b) = text(&nc.vars[k], "bounds").and_then(|b| var_index(nc, b)) {
                        structural.push(b);
                    }
                }
                return Some(Topology {
                    name: "ICON grid".into(),
                    long_name: None,
                    convention: Convention::Icon,
                    coords: Coords::LonLat,
                    face_dim: cell_dim,
                    node_dim: Some(vl.dims[0].clone()),
                    faces: cells,
                    edges: var_index(nc, "edge_vertices")
                        .map(|e| *nc.vars[e].shape.last().unwrap_or(&0)),
                    node_x: Some(vlon),
                    node_y: Some(vlat),
                    radians,
                    source: Faces::Connectivity {
                        var: voc,
                        transposed,
                        start: Some(1),
                    },
                    structural,
                });
            }
        }
    }
    // A data file: vertex bounds per cell.
    let bounds = |c: usize, alts: [&str; 2]| {
        text(&nc.vars[c], "bounds")
            .and_then(|b| var_index(nc, b))
            .or_else(|| alts.iter().find_map(|n| var_index(nc, n)))
    };
    let bl = bounds(clon, ["clon_vertices", "clon_bnds"])?;
    let bt = bounds(clat, ["clat_vertices", "clat_bnds"])?;
    let (b1, b2) = (&nc.vars[bl], &nc.vars[bt]);
    if b1.shape.len() != 2
        || b2.shape != b1.shape
        || b1.shape[0] != cells
        || !(3..=limits::MAX_FACE_NODES).contains(&b1.shape[1])
    {
        return None;
    }
    structural.extend([bl, bt]);
    Some(Topology {
        name: "ICON grid".into(),
        long_name: None,
        convention: Convention::Icon,
        coords: Coords::LonLat,
        face_dim: cell_dim,
        node_dim: None,
        faces: cells,
        edges: None,
        node_x: None,
        node_y: None,
        radians,
        source: Faces::Bounds { lon: bl, lat: bt },
        structural,
    })
}

/// Counts of faces by their number of nodes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FaceKinds {
    pub triangles: u64,
    pub quads: u64,
    /// Five nodes or more.
    pub polygons: u64,
    /// Fewer than three valid nodes, or a node without coordinates: not drawn.
    pub degenerate: u64,
    /// The most nodes of one face.
    pub max_nodes: u32,
}

/// A mesh: node positions and the faces' nodes (compressed rows).
#[derive(Clone, Debug)]
pub struct Mesh {
    /// x and y (longitude and latitude in degrees on geographic meshes); NaN for a node
    /// without coordinates.
    pub nodes: Vec<[f64; 2]>,
    /// Face `f`'s nodes are `face_nodes[offsets[f]..offsets[f + 1]]`.
    pub offsets: Vec<u32>,
    pub face_nodes: Vec<u32>,
    pub kinds: FaceKinds,
    pub coords: Coords,
    /// The `start_index` the connectivity used (stated or inferred).
    pub start_index: i64,
}

impl Mesh {
    pub fn faces(&self) -> usize {
        self.offsets.len().saturating_sub(1)
    }

    /// The nodes of face `f`.
    pub fn face(&self, f: usize) -> &[u32] {
        match (self.offsets.get(f), self.offsets.get(f + 1)) {
            (Some(&a), Some(&b)) => &self.face_nodes[a as usize..b as usize],
            _ => &[],
        }
    }

    /// Reads the mesh of a topology.
    pub fn load(
        nc: &Netcdf,
        data: &[u8],
        topo: &Topology,
        cancel: &(dyn Fn() -> bool + Sync),
    ) -> Result<Mesh> {
        let budget = (data.len() as u64)
            .max(1)
            .saturating_mul(limits::VALUES_PER_BYTE);
        let check = |n: u64, cap: u64| {
            if n > cap || n > budget {
                Err(Error::TooLarge)
            } else {
                Ok(())
            }
        };
        check(topo.faces, limits::MAX_FACES)?;
        match &topo.source {
            Faces::Connectivity {
                var,
                transposed,
                start,
            } => {
                let (Some(nx), Some(ny)) = (topo.node_x, topo.node_y) else {
                    return Err(Error::NotUgrid);
                };
                let n_nodes = nc.vars[nx].shape[0];
                check(n_nodes, limits::MAX_NODES)?;
                let xs = nc.read_all(data, nx, limits::MAX_NODES)?;
                let ys = nc.read_all(data, ny, limits::MAX_NODES)?;
                if xs.len() != ys.len() {
                    return Err(Error::Malformed("node coordinates differ in length".into()));
                }
                let k = if topo.radians {
                    180.0 / std::f64::consts::PI
                } else {
                    1.0
                };
                let nodes: Vec<[f64; 2]> =
                    xs.iter().zip(&ys).map(|(&x, &y)| [x * k, y * k]).collect();
                let c = &nc.vars[*var];
                let (faces, nmax) = if *transposed {
                    (c.shape[1], c.shape[0])
                } else {
                    (c.shape[0], c.shape[1])
                };
                check(faces.saturating_mul(nmax), limits::MAX_CONNECTIVITY)?;
                let raw = read_connectivity(nc, data, *var, *transposed, faces, nmax, cancel)?;
                let start = settle_start(*start, &raw, nodes.len());
                let (offsets, face_nodes) = rows(&raw, nmax as usize, start, nodes.len())?;
                let mut mesh = Mesh {
                    nodes,
                    offsets,
                    face_nodes,
                    kinds: FaceKinds::default(),
                    coords: topo.coords.clone(),
                    start_index: start,
                };
                mesh.count_kinds();
                Ok(mesh)
            }
            Faces::Bounds { lon, lat } => {
                let shape = &nc.vars[*lon].shape;
                let (faces, nmax) = (shape[0], shape[1]);
                check(faces.saturating_mul(nmax), limits::MAX_CONNECTIVITY)?;
                let lons = nc.read_all(data, *lon, limits::MAX_CONNECTIVITY)?;
                let lats = nc.read_all(data, *lat, limits::MAX_CONNECTIVITY)?;
                if cancel() {
                    return Err(Error::Cancelled);
                }
                let k = if topo.radians {
                    180.0 / std::f64::consts::PI
                } else {
                    1.0
                };
                let mut index =
                    std::collections::HashMap::<(u64, u64), u32>::with_capacity(lons.len() / 2);
                let mut nodes = Vec::new();
                let mut offsets = Vec::with_capacity(faces as usize + 1);
                let mut face_nodes = Vec::with_capacity(lons.len());
                offsets.push(0u32);
                for (lo, la) in lons
                    .chunks_exact(nmax as usize)
                    .zip(lats.chunks_exact(nmax as usize))
                {
                    let mut prev = u32::MAX;
                    let first = face_nodes.len();
                    for (&x, &y) in lo.iter().zip(la) {
                        if !x.is_finite() || !y.is_finite() {
                            continue;
                        }
                        let key = (x.to_bits(), y.to_bits());
                        let id = *index.entry(key).or_insert_with(|| {
                            nodes.push([x * k, y * k]);
                            (nodes.len() - 1) as u32
                        });
                        // Repeated vertices pad cells with fewer nodes.
                        if id != prev {
                            face_nodes.push(id);
                        }
                        prev = id;
                    }
                    if face_nodes.len() > first + 1 && face_nodes[first] == prev {
                        face_nodes.pop();
                    }
                    offsets.push(u32::try_from(face_nodes.len()).map_err(|_| Error::TooLarge)?);
                }
                check(nodes.len() as u64, limits::MAX_NODES)?;
                let mut mesh = Mesh {
                    nodes,
                    offsets,
                    face_nodes,
                    kinds: FaceKinds::default(),
                    coords: Coords::LonLat,
                    start_index: 0,
                };
                mesh.count_kinds();
                Ok(mesh)
            }
        }
    }

    fn count_kinds(&mut self) {
        let mut k = FaceKinds::default();
        for f in 0..self.faces() {
            let nodes = self.face(f);
            k.max_nodes = k.max_nodes.max(nodes.len() as u32);
            let ok = nodes.len() >= 3
                && nodes.iter().all(|&n| {
                    let p = self.nodes[n as usize];
                    p[0].is_finite() && p[1].is_finite()
                });
            match nodes.len() {
                _ if !ok => k.degenerate += 1,
                3 => k.triangles += 1,
                4 => k.quads += 1,
                _ => k.polygons += 1,
            }
        }
        self.kinds = k;
    }

    /// Builds a mesh from parts (tests, synthetic meshes): `faces` lists each face's
    /// 0-based nodes.
    pub fn from_parts(nodes: Vec<[f64; 2]>, faces: &[&[u32]], coords: Coords) -> Result<Mesh> {
        let mut offsets = vec![0u32];
        let mut face_nodes = Vec::new();
        for f in faces {
            if f.iter().any(|&n| n as usize >= nodes.len()) {
                return Err(Error::Malformed("face node index out of range".into()));
            }
            face_nodes.extend_from_slice(f);
            offsets.push(face_nodes.len() as u32);
        }
        let mut m = Mesh {
            nodes,
            offsets,
            face_nodes,
            kinds: FaceKinds::default(),
            coords,
            start_index: 0,
        };
        m.count_kinds();
        Ok(m)
    }
}

/// Reads the connectivity in blocks of faces, as `faces` rows of `nmax` stored values
/// (NaN where the fill value stands).
fn read_connectivity(
    nc: &Netcdf,
    data: &[u8],
    var: usize,
    transposed: bool,
    faces: u64,
    nmax: u64,
    cancel: &(dyn Fn() -> bool + Sync),
) -> Result<Vec<f64>> {
    let mut out = Vec::with_capacity((faces * nmax) as usize);
    let mut f0 = 0;
    while f0 < faces {
        if cancel() {
            return Err(Error::Cancelled);
        }
        let n = limits::BLOCK.min(faces - f0);
        if transposed {
            let (block, _) = nc.read_box(data, var, &[0, f0], &[nmax, n], cancel)?;
            let n = n as usize;
            for f in 0..n {
                for k in 0..nmax as usize {
                    out.push(block[k * n + f]);
                }
            }
        } else {
            let (block, _) = nc.read_box(data, var, &[f0, 0], &[n, nmax], cancel)?;
            out.extend_from_slice(&block);
        }
        f0 += n;
    }
    Ok(out)
}

/// The start index a connectivity without `start_index` uses: 1 when no index is 0 and
/// the largest equals the node count (FVCOM's `nv`, ADCIRC's `element`), else 0.
fn infer_start(raw: &[f64], nodes: usize) -> i64 {
    let (lo, hi) = index_range(raw);
    i64::from(lo >= 1.0 && hi == nodes as f64)
}

/// The smallest and largest non-negative index.
fn index_range(raw: &[f64]) -> (f64, f64) {
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for &v in raw {
        if v.is_finite() && v >= 0.0 {
            lo = lo.min(v);
            hi = hi.max(v);
        }
    }
    (lo, hi)
}

/// The start index to use: the stated one unless the indices plainly contradict it (some
/// writers state `start_index = 1` over 0-based indices, which then use node 0 and never
/// the last node; or the reverse), else inferred as FVCOM writes `nv`.
fn settle_start(stated: Option<i64>, raw: &[f64], nodes: usize) -> i64 {
    let Some(s) = stated else {
        return infer_start(raw, nodes);
    };
    let (lo, hi) = index_range(raw);
    let n = nodes as f64;
    match s {
        1 if lo == 0.0 && hi <= n - 1.0 => 0,
        0 if lo >= 1.0 && hi == n => 1,
        _ => s,
    }
}

/// Turns connectivity rows into compressed rows of 0-based node ids. Values below the
/// start index (negative fills such as -1 or -999 without a `_FillValue`) and masked ones
/// end a face's list; an index past the last node is an error.
fn rows(raw: &[f64], nmax: usize, start: i64, nodes: usize) -> Result<(Vec<u32>, Vec<u32>)> {
    let faces = raw.len() / nmax.max(1);
    let mut offsets = Vec::with_capacity(faces + 1);
    let mut out = Vec::with_capacity(raw.len());
    offsets.push(0u32);
    for row in raw.chunks_exact(nmax.max(1)) {
        let first = out.len();
        for &v in row {
            if !v.is_finite() {
                continue;
            }
            if v.fract() != 0.0 {
                return Err(Error::Malformed("fractional node index".into()));
            }
            let i = v - start as f64;
            if i < 0.0 {
                continue;
            }
            if i >= nodes as f64 {
                return Err(Error::Malformed(format!(
                    "face node index {v} past the {nodes} nodes"
                )));
            }
            let id = i as u32;
            // Closing repeats of the first node (some writers close polygons).
            if out.len() > first && (out.last() == Some(&id)) {
                continue;
            }
            out.push(id);
        }
        if out.len() > first + 1 && out[first] == *out.last().unwrap_or(&u32::MAX) {
            out.pop();
        }
        offsets.push(u32::try_from(out.len()).map_err(|_| Error::TooLarge)?);
    }
    Ok((offsets, out))
}

/// A data variable on the mesh.
#[derive(Clone, Debug)]
pub struct DataVar {
    /// The variable's index in the file.
    pub var: usize,
    pub location: Location,
    /// Which of the variable's dimensions runs along the faces or nodes.
    pub element_dim: usize,
    /// The variable's other dimensions with more than one position, in order: the
    /// cube's axes.
    pub axes: Vec<usize>,
}

/// The numeric variables with values on the faces or nodes of a topology, in file order:
/// those that name the mesh (`mesh` and `location`), and those that merely run along its
/// face or node dimension (FVCOM and older writers).
pub fn variables(nc: &Netcdf, topo: &Topology) -> Vec<DataVar> {
    // Auxiliary coordinates other variables name.
    let coordinates: Vec<&str> = nc
        .vars
        .iter()
        .filter_map(|v| text(v, "coordinates"))
        .flat_map(str::split_whitespace)
        .collect();
    let mut out = Vec::new();
    for (i, v) in nc.vars.iter().enumerate() {
        if v.num.is_none() || topo.structural.contains(&i) || v.attr("cf_role").is_some() {
            continue;
        }
        let sn = text(v, "standard_name").unwrap_or("");
        if sn == "longitude"
            || sn == "latitude"
            || sn.ends_with("_coordinate")
            || coordinates.contains(&v.name.as_str())
        {
            continue;
        }
        let named = text(v, "mesh");
        if named.is_some_and(|m| m != topo.name) {
            continue;
        }
        let loc = text(v, "location");
        let dim_of = |d: &str| v.dims.iter().position(|x| x == d);
        let (location, k) = match loc {
            Some("face") => match dim_of(&topo.face_dim) {
                Some(k) => (Location::Face, k),
                None => continue,
            },
            Some("node") => match topo.node_dim.as_deref().and_then(dim_of) {
                Some(k) => (Location::Node, k),
                None => continue,
            },
            Some(_) => continue,
            None => match (
                dim_of(&topo.face_dim),
                topo.node_dim.as_deref().and_then(dim_of),
            ) {
                (Some(k), None) => (Location::Face, k),
                (None, Some(k)) => (Location::Node, k),
                _ => continue,
            },
        };
        // The element dimension appears once; the variable is not a coordinate of it.
        if v.dims.iter().filter(|d| **d == v.dims[k]).count() != 1
            || (v.dims.len() == 1 && v.name == v.dims[0])
        {
            continue;
        }
        let axes = (0..v.dims.len())
            .filter(|&d| d != k && v.shape[d] > 1)
            .collect();
        out.push(DataVar {
            var: i,
            location,
            element_dim: k,
            axes,
        });
    }
    out
}

/// Reads a data variable's values at a position on its axes (`index`, one entry per
/// axis), one per face or node, as f32 (NaN where masked).
pub fn read_values(
    nc: &Netcdf,
    data: &[u8],
    dv: &DataVar,
    index: &[u32],
    cancel: &(dyn Fn() -> bool + Sync),
) -> Result<(Vec<f32>, bool)> {
    let v = nc
        .vars
        .get(dv.var)
        .ok_or_else(|| Error::Malformed("no such variable".into()))?;
    let r = v.shape.len();
    let mut start = vec![0u64; r];
    let mut count = vec![1u64; r];
    count[dv.element_dim] = v.shape[dv.element_dim];
    for (a, &d) in dv.axes.iter().enumerate() {
        let i = u64::from(index.get(a).copied().unwrap_or(0));
        if i >= v.shape[d] {
            return Err(Error::Malformed("position past the axis".into()));
        }
        start[d] = i;
    }
    let (vals, truncated) = nc.read_box(data, dv.var, &start, &count, cancel)?;
    Ok((vals.into_iter().map(|x| x as f32).collect(), truncated))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_handle_fill_start_and_range() {
        // Two faces, max 4 nodes, 1-based, -1 and NaN fills.
        let raw = [1.0, 2.0, 3.0, -1.0, 2.0, 3.0, 4.0, 5.0];
        let (off, nodes) = rows(&raw, 4, 1, 5).unwrap();
        assert_eq!(off, vec![0, 3, 7]);
        assert_eq!(nodes, vec![0, 1, 2, 1, 2, 3, 4]);
        let raw = [0.0, 1.0, 2.0, f64::NAN];
        assert_eq!(rows(&raw, 4, 0, 3).unwrap().0, vec![0, 3]);
        // Out of range: refused.
        assert!(rows(&[0.0, 1.0, 9.0], 3, 0, 3).is_err());
        // A closed polygon drops its repeat.
        let (off, _) = rows(&[0.0, 1.0, 2.0, 0.0], 4, 0, 3).unwrap();
        assert_eq!(off, vec![0, 3]);
        assert_eq!(infer_start(&[1.0, 2.0, 3.0], 3), 1);
        assert_eq!(infer_start(&[0.0, 1.0, 2.0], 3), 0);
        // A stated start contradicted by the indices (NOAA gridded's quad_and_triangle).
        assert_eq!(
            settle_start(Some(1), &[0.0, 1.0, 2.0, f64::NAN, 1.0, 3.0, 4.0, 2.0], 5),
            0
        );
        assert_eq!(settle_start(Some(1), &[1.0, 2.0, 5.0], 5), 1);
        assert_eq!(settle_start(Some(0), &[1.0, 2.0, 3.0], 3), 1);
        assert_eq!(settle_start(Some(0), &[0.0, 1.0, 2.0], 3), 0);
    }

    #[test]
    fn kinds() {
        let nodes = vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0], [2.0, 0.5]];
        let m = Mesh::from_parts(
            nodes,
            &[&[0, 1, 2], &[0, 1, 2, 3], &[0, 1, 4, 2, 3], &[0, 1]],
            Coords::LonLat,
        )
        .unwrap();
        assert_eq!(
            m.kinds,
            FaceKinds {
                triangles: 1,
                quads: 1,
                polygons: 1,
                degenerate: 1,
                max_nodes: 5
            }
        );
    }
}
