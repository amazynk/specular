//! VTK datasets through `specular-vtk`: legacy `.vtk` and XML `.vti`, `.vtr`, `.vts`, `.vtu`,
//! `.vtp`.
//!
//! * **Grids** (structured points / image data, rectilinear and structured grids) are pages
//!   on the array session ([`crate::array_dec`]): each point array, then each cell array, an
//!   image or a volume with z on a slider, y up as VTK draws it. Arrays of several
//!   components get a component slider; colour scalars of 3 or 4 components are shown in
//!   colour. Image data's origin and spacing (and evenly spaced rectilinear coordinates) give
//!   the pixel positions; a structured grid's values are shown by index.
//! * **Polygonal data and unstructured grids** are meshes on the mesh path: polygons and
//!   strips as triangles, 3-D cells by their boundary faces, coloured through viridis by the
//!   first point scalars (or the first cell scalars, faces flat), colour scalars as they are.

use std::sync::Arc;

use specular_arrays::{Part, PlaneSpec};
use specular_core::Source;
use specular_vtk::{self as vtk, DataArray, Dataset, Grid, MeshData};

use crate::array_dec::{self, Convention, Page, PlaneRead, Shared};
use crate::science::Linear;
use crate::{DecodeError, Decoder, Format, Result, Session};

fn map_err(e: vtk::Error) -> DecodeError {
    match e {
        vtk::Error::NotVtk => DecodeError::UnsupportedFormat,
        vtk::Error::Truncated(s) => DecodeError::Truncated(s),
        vtk::Error::Malformed(s) => DecodeError::Malformed(s),
        vtk::Error::Unsupported(s) => DecodeError::Unsupported(s),
        vtk::Error::TooLarge => DecodeError::TooLarge,
    }
}

pub struct VtkDecoder;

impl Decoder for VtkDecoder {
    fn format(&self) -> Format {
        Format::Vtk
    }
    fn probe(&self, head: &[u8]) -> bool {
        vtk::sniff(head).is_some() || vtk::sniff_other_xml(head)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["vtk", "vti", "vtr", "vts", "vtu", "vtp"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let mut v = vtk::parse(source.bytes()).map_err(map_err)?;
        match std::mem::replace(&mut v.dataset, Dataset::Mesh(MeshData::default())) {
            Dataset::Grid(g) => open_grid(source, &v, g),
            Dataset::Mesh(m) => open_mesh(source, v.title, m),
        }
    }
}

/// Values in memory with explicit element strides.
struct VecRead {
    values: Arc<Vec<f32>>,
    shape: Vec<u64>,
    strides: Vec<u64>,
}

impl PlaneRead for VecRead {
    fn read(&self, spec: &PlaneSpec, _part: Part, out: &mut Vec<f32>) -> Result<bool> {
        let (w, h) = spec.check(&self.shape).map_err(crate::array_dec::arr_err)?;
        let base = spec.base(&self.strides) as usize;
        let sx = self.strides[spec.x_dim] as usize;
        let sy = spec.y_dim.map_or(0, |d| self.strides[d] as usize);
        out.clear();
        out.reserve(w * h);
        let mut truncated = false;
        for y in 0..h {
            for x in 0..w {
                match self.values.get(base + y * sy + x * sx) {
                    Some(&v) => out.push(v),
                    None => {
                        truncated = true;
                        out.push(f32::NAN);
                    }
                }
            }
        }
        Ok(truncated)
    }
}

/// Evenly spaced coordinates as a calibration.
fn even(c: &[f64]) -> Option<Linear> {
    match c {
        [] => None,
        [a] => Some(Linear {
            offset: *a,
            step: 1.0,
        }),
        [a, b, ..] => {
            let step = b - a;
            let ok = c
                .windows(2)
                .all(|w| ((w[1] - w[0]) - step).abs() <= 1e-6 * step.abs().max(1e-30));
            (ok && step != 0.0).then_some(Linear { offset: *a, step })
        }
    }
}

fn axis_labels(n: usize) -> Vec<String> {
    let names = ["X", "Y", "Z"];
    (0..n)
        .map(|i| {
            if n <= 3 {
                names[i].to_owned()
            } else {
                (i + 1).to_string()
            }
        })
        .collect()
}

fn grid_page(a: DataArray, dims: [usize; 3], cal: [Option<Linear>; 3], cell: bool) -> Option<Page> {
    let [nx, ny, nz] = dims.map(|d| d as u64);
    let c = a.components.max(1) as u64;
    if a.values.len() as u64 != nx * ny * nz * c {
        return None;
    }
    let colour = a.attribute == Some("COLOR_SCALARS") && (c == 3 || c == 4);
    let (shape, strides) = if c == 1 {
        (vec![nz, ny, nx], vec![ny * nx, nx, 1])
    } else if colour {
        (vec![nz, ny, nx, c], vec![ny * nx * c, nx * c, c, 1])
    } else {
        // Components on a slider, ahead of z.
        (vec![c, nz, ny, nx], vec![1, ny * nx * c, nx * c, c])
    };
    let label = if cell {
        format!("{} (cells)", a.name)
    } else {
        a.name.clone()
    };
    let reader = Arc::new(VecRead {
        values: Arc::new(a.values),
        shape: shape.clone(),
        strides,
    });
    let mut p = Page::new(label, shape.clone(), Convention::LastIsX, reader);
    let shape_text = if c > 1 {
        format!("{} × {} components", a.dtype, c)
    } else {
        a.dtype.to_owned()
    };
    p.class = shape_text;
    p.rgb_ok = colour;
    p.bits = 32;
    p.sample = format!("{}, shown as 32-bit float", a.dtype);
    p.flip_y = true;
    let off = if c == 1 || colour { 0 } else { 1 };
    // Cell values sit at the centres between points.
    let shift = |l: Option<Linear>| {
        l.map(|l| {
            if cell {
                Linear {
                    offset: l.offset + l.step / 2.0,
                    step: l.step,
                }
            } else {
                l
            }
        })
    };
    p.dim_cal[off + 2] = shift(cal[0]).map(|l| (l, String::new()));
    p.dim_cal[off + 1] = shift(cal[1]).map(|l| (l, String::new()));
    p.dim_cal[off] = shift(cal[2]).map(|l| (l, String::new()));
    p.dim_names[off] = Some("z".to_owned());
    if c > 1 && !colour {
        p.dim_labels[0] = axis_labels(c as usize);
        p.dim_names[0] = Some(
            a.attribute
                .map_or("Component".to_owned(), |r| r.to_ascii_lowercase()),
        );
    }
    if let Some(r) = a.attribute {
        p.keywords
            .push(("attribute".into(), r.to_owned(), String::new()));
    }
    p.keywords.push((
        "located on".into(),
        if cell { "cells" } else { "points" }.into(),
        String::new(),
    ));
    Some(p)
}

fn header(shared: &mut Shared, v: &vtk::Vtk) {
    shared.kw("VTK file", format!("{} {}", v.kind.name(), v.version));
    shared.kw("encoding", v.encoding.clone());
    if let Some(t) = v.title.as_ref().filter(|t| !t.trim().is_empty()) {
        shared.kw("title", t.clone());
    }
    for f in &v.field_data {
        let vals: Vec<String> = f.values.iter().take(16).map(|x| x.to_string()).collect();
        shared.kw(f.name.clone(), vals.join(", "));
    }
}

fn open_grid(source: Arc<Source>, v: &vtk::Vtk, g: Grid) -> Result<Arc<dyn Session>> {
    let mut shared = Shared::new(Format::Vtk);
    header(&mut shared, v);
    shared.kw("structure", g.structure);
    shared.kw(
        "points",
        format!("{} × {} × {}", g.dims[0], g.dims[1], g.dims[2]),
    );
    let cal: [Option<Linear>; 3] = match (&g.coords, &g.points) {
        (Some(c), _) => [even(&c[0]), even(&c[1]), even(&c[2])],
        // A curvilinear grid: by index.
        (None, Some(_)) => [None, None, None],
        (None, None) => [0, 1, 2].map(|i| {
            (g.spacing[i].is_finite() && g.spacing[i] != 0.0 && g.origin[i].is_finite()).then(
                || Linear {
                    offset: g.origin[i],
                    step: g.spacing[i],
                },
            )
        }),
    };
    if g.points.is_some() {
        shared.kw("note", "a curvilinear grid: values are shown by index");
    }
    let cdims = g.cell_dims();
    for a in g.point_data {
        if let Some(p) = grid_page(a, g.dims, cal, false) {
            shared.pages.push(p);
        }
    }
    for a in g.cell_data {
        if let Some(p) = grid_page(a, cdims, cal, true) {
            shared.pages.push(p);
        }
    }
    array_dec::session(source, shared, "the VTK grid holds no data arrays")
}

/// Colours per vertex through viridis between the values' minimum and maximum, or the
/// colour scalars themselves.
fn colours(a: &DataArray) -> Option<Vec<[u8; 4]>> {
    let c = a.components.max(1);
    let n = a.tuples();
    if a.attribute == Some("COLOR_SCALARS") && (c == 3 || c == 4) {
        return Some(
            (0..n)
                .map(|i| {
                    let t = &a.values[i * c..i * c + c];
                    let b = |v: f32| v.clamp(0.0, 255.0) as u8;
                    [
                        b(t[0]),
                        b(t[1]),
                        b(t[2]),
                        if c == 4 { b(t[3]) } else { 255 },
                    ]
                })
                .collect(),
        );
    }
    let first: Vec<f32> = (0..n).map(|i| a.values[i * c]).collect();
    let (lo, hi) = first
        .iter()
        .filter(|v| v.is_finite())
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(l, h), &v| {
            (l.min(v), h.max(v))
        });
    if !(lo.is_finite() && hi > lo) {
        return None;
    }
    let lut = crate::imzml_dec::viridis_lut();
    Some(
        first
            .iter()
            .map(|&v| {
                let i = if v.is_finite() {
                    (((v - lo) / (hi - lo)) * 255.0).round().clamp(0.0, 255.0) as usize
                } else {
                    0
                };
                let c = lut[i];
                [c[0], c[1], c[2], 255]
            })
            .collect(),
    )
}

/// The array to colour by: the active scalars, else the first single-component array.
fn colour_array(arrays: &[DataArray]) -> Option<&DataArray> {
    arrays
        .iter()
        .find(|a| matches!(a.attribute, Some("SCALARS" | "Scalars" | "COLOR_SCALARS")))
        .or_else(|| arrays.iter().find(|a| a.components == 1))
}

fn open_mesh(source: Arc<Source>, title: Option<String>, m: MeshData) -> Result<Arc<dyn Session>> {
    if m.triangles.is_empty() {
        return Err(DecodeError::Unsupported(if m.lines + m.vertices > 0 {
            "this VTK file holds only points or lines, no surfaces to show".into()
        } else {
            "this VTK file holds no cells to show".into()
        }));
    }
    let limits = specular_mesh::Limits::default();
    let point_colours = colour_array(&m.point_data)
        .filter(|a| a.tuples() == m.points.len())
        .and_then(colours);
    let cell_colours = if point_colours.is_none() {
        colour_array(&m.cell_data).and_then(colours)
    } else {
        None
    };
    let mut mesh = match cell_colours {
        // Faces coloured by their cell: each triangle gets vertices of its own.
        Some(cc) if m.triangles.len() * 3 <= limits.max_vertices => {
            let mut pos = Vec::with_capacity(m.triangles.len() * 3);
            let mut col = Vec::with_capacity(m.triangles.len() * 3);
            for (t, &cell) in m.triangles.iter().zip(&m.triangle_cell) {
                let c = cc.get(cell as usize).copied().unwrap_or([255; 4]);
                for &i in t {
                    pos.push(m.points.get(i as usize).copied().unwrap_or([0.0; 3]));
                    col.push(c);
                }
            }
            let idx: Vec<u32> = (0..pos.len() as u32).collect();
            specular_mesh::from_triangles(pos, idx, Some(col), &limits)
        }
        _ => specular_mesh::from_triangles(
            m.points.clone(),
            m.triangles.iter().flatten().copied().collect(),
            point_colours,
            &limits,
        ),
    }
    .map_err(|e| match e {
        specular_mesh::Error::TooLarge(_) => DecodeError::TooLarge,
        specular_mesh::Error::Unsupported(s) | specular_mesh::Error::Malformed(s) => {
            DecodeError::Malformed(s)
        }
        specular_mesh::Error::Empty => DecodeError::Malformed("no triangles".into()),
    })?;
    // ParaView's default view: y up.
    mesh.up = specular_mesh::Up::Y;
    mesh.title = title.filter(|t| !t.trim().is_empty());
    mesh.notes.extend(m.notes);
    if m.vertices + m.lines > 0 {
        mesh.notes.push(format!(
            "{} vertex and {} line cells not drawn",
            m.vertices, m.lines
        ));
    }
    Ok(crate::mesh_dec::session(source, Format::Vtk, mesh))
}
