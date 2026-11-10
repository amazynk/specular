"""Writes the synthetic UGRID and ICON fixtures under tests/data (small, committed) and,
with --big DIR, a 1.3M-face global mesh for the timing and GPU checks.

Run from a venv with netCDF4 and numpy:
    python gen/make_ugrid.py tests/data [--big DIR]
"""

import os
import sys

import numpy as np
from netCDF4 import Dataset

T_UNITS = "hours since 2020-01-01 00:00:00"


def mesh_var(nc, name, x, y, conn, start=None, fill=None, face_dim="nMesh2_face"):
    m = nc.createVariable(name, "i4")
    m.cf_role = "mesh_topology"
    m.long_name = "Topology data of 2D unstructured mesh"
    m.topology_dimension = 2
    m.node_coordinates = f"{x} {y}"
    m.face_node_connectivity = conn
    m.face_dimension = face_dim
    return m


def tri_start0(path):
    """Triangles, start_index 0, projected metres, face data over time, node data."""
    nx, ny = 7, 6
    rng = np.random.default_rng(1)
    xs, ys = [], []
    for j in range(ny):
        for i in range(nx):
            jit = rng.uniform(-0.2, 0.2, 2) if 0 < i < nx - 1 and 0 < j < ny - 1 else (0, 0)
            xs.append(500000.0 + 1000.0 * (i + jit[0]))
            ys.append(4100000.0 + 1000.0 * (j + jit[1]))
    tris = []
    for j in range(ny - 1):
        for i in range(nx - 1):
            a = j * nx + i
            if (i + j) % 2:
                tris += [[a, a + 1, a + nx + 1], [a, a + nx + 1, a + nx]]
            else:
                tris += [[a, a + 1, a + nx], [a + 1, a + nx + 1, a + nx]]
    tris = np.array(tris, dtype="i4")
    nc = Dataset(path, "w", format="NETCDF3_CLASSIC")
    nc.Conventions = "CF-1.8 UGRID-1.0"
    nc.title = "Synthetic triangle mesh"
    nc.createDimension("nMesh2_node", len(xs))
    nc.createDimension("nMesh2_face", len(tris))
    nc.createDimension("nMaxMesh2_face_nodes", 3)
    nc.createDimension("time", 3)
    mesh_var(nc, "Mesh2", "Mesh2_node_x", "Mesh2_node_y", "Mesh2_face_nodes")
    v = nc.createVariable("Mesh2_node_x", "f8", ("nMesh2_node",))
    v.standard_name = "projection_x_coordinate"
    v.units = "m"
    v[:] = xs
    v = nc.createVariable("Mesh2_node_y", "f8", ("nMesh2_node",))
    v.standard_name = "projection_y_coordinate"
    v.units = "m"
    v[:] = ys
    v = nc.createVariable("Mesh2_face_nodes", "i4", ("nMesh2_face", "nMaxMesh2_face_nodes"))
    v.cf_role = "face_node_connectivity"
    v.start_index = 0
    v[:] = tris
    t = nc.createVariable("time", "f8", ("time",))
    t.units = T_UNITS
    t.calendar = "standard"
    t.standard_name = "time"
    t[:] = [0, 6, 12]
    d = nc.createVariable("depth", "f4", ("time", "nMesh2_face"))
    d.mesh = "Mesh2"
    d.location = "face"
    d.units = "m"
    d.long_name = "Water depth"
    cx = np.array(xs)[tris].mean(1)
    cy = np.array(ys)[tris].mean(1)
    d[:] = np.stack([(cx - 500000) / 100 + 3 * k + (cy - 4100000) / 300 for k in range(3)])
    b = nc.createVariable("bed", "f8", ("nMesh2_node",))
    b.mesh = "Mesh2"
    b.location = "node"
    b.units = "m"
    b[:] = (np.array(xs) - 500000) / 250 + np.sin((np.array(ys) - 4100000) / 1500)
    nc.close()


def mixed_fill_start1(path):
    """Quads, triangles and a pentagon: start_index 1, _FillValue -999, lon/lat, node
    data over time and face data over time and depth layers (NetCDF-4)."""
    nx, ny = 8, 6
    xs, ys = [], []
    for j in range(ny):
        for i in range(nx):
            xs.append(4.0 + 0.1 * i + 0.01 * ((i * 3 + j) % 4))
            ys.append(52.0 + 0.08 * j)
    faces = []
    for j in range(ny - 1):
        for i in range(nx - 1):
            a = j * nx + i
            if i == nx - 2 and j == 2:
                # A pentagon bulging past the east edge.
                xs.append(xs[a + 1] + 0.04)
                ys.append((ys[a + 1] + ys[a + nx + 1]) / 2)
                m = len(xs) - 1
                faces.append([a, a + 1, m, a + nx + 1, a + nx])
            elif (i + 2 * j) % 3 == 0:
                faces += [[a, a + 1, a + nx + 1], [a, a + nx + 1, a + nx]]
            else:
                faces.append([a, a + 1, a + nx + 1, a + nx])
    conn = np.full((len(faces), 5), -999, dtype="i4")
    for k, f in enumerate(faces):
        conn[k, : len(f)] = np.array(f) + 1
    nc = Dataset(path, "w", format="NETCDF4")
    nc.Conventions = "CF-1.8 UGRID-1.0"
    nc.institution = "Specular test suite"
    nc.createDimension("mesh2d_nNodes", len(xs))
    nc.createDimension("mesh2d_nFaces", len(faces))
    nc.createDimension("mesh2d_nMax_face_nodes", 5)
    nc.createDimension("time", 2)
    nc.createDimension("layer", 3)
    m = mesh_var(
        nc, "mesh2d", "mesh2d_node_x", "mesh2d_node_y", "mesh2d_face_nodes",
        face_dim="mesh2d_nFaces",
    )
    m.long_name = "Delft3D-FM style mesh"
    v = nc.createVariable("mesh2d_node_x", "f8", ("mesh2d_nNodes",))
    v.standard_name = "longitude"
    v.units = "degrees_east"
    v[:] = xs
    v = nc.createVariable("mesh2d_node_y", "f8", ("mesh2d_nNodes",))
    v.standard_name = "latitude"
    v.units = "degrees_north"
    v[:] = ys
    v = nc.createVariable(
        "mesh2d_face_nodes", "i4", ("mesh2d_nFaces", "mesh2d_nMax_face_nodes"),
        fill_value=-999,
    )
    v.cf_role = "face_node_connectivity"
    v.start_index = 1
    v[:] = conn
    t = nc.createVariable("time", "f8", ("time",))
    t.units = T_UNITS
    t[:] = [0, 24]
    z = nc.createVariable("layer", "f8", ("layer",))
    z.units = "m"
    z.positive = "down"
    z.standard_name = "depth"
    z[:] = [0.5, 2.0, 5.0]
    xs, ys = np.array(xs), np.array(ys)
    w = nc.createVariable("mesh2d_s1", "f4", ("time", "mesh2d_nNodes"), fill_value=-999.0)
    w.mesh = "mesh2d"
    w.location = "node"
    w.units = "m"
    w.standard_name = "sea_surface_height"
    w[:] = np.stack([np.sin(xs * 20 + k) + ys - 52 for k in range(2)])
    cx = np.array([xs[f].mean() for f in faces])
    cy = np.array([ys[f].mean() for f in faces])
    s = nc.createVariable("mesh2d_sa1", "f4", ("time", "layer", "mesh2d_nFaces"), fill_value=-999.0)
    s.mesh = "mesh2d"
    s.location = "face"
    s.units = "1e-3"
    s.long_name = "Salinity"
    vals = np.stack(
        [np.stack([30 + cx * 0.5 + cy * 0.1 + k + 0.2 * z for z in range(3)]) for k in range(2)]
    )
    vals[0, 0, 3] = -999.0
    s[:] = vals
    nc.close()


def icosphere(level):
    """Vertices (unit vectors) and triangles of a subdivided icosahedron."""
    t = (1 + 5**0.5) / 2
    v = [[-1, t, 0], [1, t, 0], [-1, -t, 0], [1, -t, 0], [0, -1, t], [0, 1, t],
         [0, -1, -t], [0, 1, -t], [t, 0, -1], [t, 0, 1], [-t, 0, -1], [-t, 0, 1]]
    f = [[0, 11, 5], [0, 5, 1], [0, 1, 7], [0, 7, 10], [0, 10, 11], [1, 5, 9], [5, 11, 4],
         [11, 10, 2], [10, 7, 6], [7, 1, 8], [3, 9, 4], [3, 4, 2], [3, 2, 6], [3, 6, 8],
         [3, 8, 9], [4, 9, 5], [2, 4, 11], [6, 2, 10], [8, 6, 7], [9, 8, 1]]
    v = np.array(v, dtype="f8")
    v /= np.linalg.norm(v, axis=1)[:, None]
    # Tilt so no vertex sits exactly on a pole or the antimeridian.
    a = 0.3
    rot = np.array([[np.cos(a), 0, np.sin(a)], [0, 1, 0], [-np.sin(a), 0, np.cos(a)]])
    v = v @ rot.T
    f = np.array(f, dtype="i8")
    for _ in range(level):
        edges = np.concatenate([f[:, [0, 1]], f[:, [1, 2]], f[:, [2, 0]]])
        key = np.sort(edges, axis=1)
        uniq, inv = np.unique(key, axis=0, return_inverse=True)
        mid = v[uniq[:, 0]] + v[uniq[:, 1]]
        mid /= np.linalg.norm(mid, axis=1)[:, None]
        base = len(v)
        v = np.concatenate([v, mid])
        n = len(f)
        m01, m12, m20 = (inv[:n] + base, inv[n : 2 * n] + base, inv[2 * n :] + base)
        f = np.concatenate([
            np.stack([f[:, 0], m01, m20], 1),
            np.stack([f[:, 1], m12, m01], 1),
            np.stack([f[:, 2], m20, m12], 1),
            np.stack([m01, m12, m20], 1),
        ])
    lon = np.arctan2(v[:, 1], v[:, 0])
    lat = np.arcsin(np.clip(v[:, 2], -1, 1))
    return v, f, lon, lat


def icon(path_grid, path_data, level=2):
    """An ICON-style grid file (vertex_of_cell over vlon/vlat, radians) and a data file
    with only cell centres and vertex bounds."""
    v, f, lon, lat = icosphere(level)
    c = v[f].mean(1)
    c /= np.linalg.norm(c, axis=1)[:, None]
    clon, clat = np.arctan2(c[:, 1], c[:, 0]), np.arcsin(c[:, 2])
    nc = Dataset(path_grid, "w", format="NETCDF4")
    nc.title = "ICON grid description"
    nc.grid_level = level
    nc.createDimension("cell", len(f))
    nc.createDimension("vertex", len(v))
    nc.createDimension("nv", 3)
    for name, dim, vals, sn in [
        ("clon", "cell", clon, "longitude"), ("clat", "cell", clat, "latitude"),
        ("vlon", "vertex", lon, "longitude"), ("vlat", "vertex", lat, "latitude"),
    ]:
        x = nc.createVariable(name, "f8", (dim,))
        x.units = "radian"
        x.standard_name = sn
        x[:] = vals
    voc = nc.createVariable("vertex_of_cell", "i4", ("nv", "cell"))
    voc[:] = f.T + 1
    area = nc.createVariable("cell_area", "f8", ("cell",))
    area.units = "m2"
    a = np.linalg.norm(np.cross(v[f[:, 1]] - v[f[:, 0]], v[f[:, 2]] - v[f[:, 0]]), axis=1) / 2
    area[:] = a * 6371229.0**2
    nc.close()

    nc = Dataset(path_data, "w", format="NETCDF4")
    nc.createDimension("ncells", len(f))
    nc.createDimension("vertices", 3)
    nc.createDimension("time", 2)
    for name, vals, sn in [("clon", clon, "longitude"), ("clat", clat, "latitude")]:
        x = nc.createVariable(name, "f8", ("ncells",))
        x.units = "radian"
        x.standard_name = sn
        x.bounds = name + "_bnds"
        x[:] = vals
    nc.createVariable("clon_bnds", "f8", ("ncells", "vertices"))[:] = lon[f]
    nc.createVariable("clat_bnds", "f8", ("ncells", "vertices"))[:] = lat[f]
    t = nc.createVariable("time", "f8", ("time",))
    t.units = T_UNITS
    t[:] = [0, 3]
    ts = nc.createVariable("ts", "f4", ("time", "ncells"))
    ts.units = "K"
    ts.standard_name = "surface_temperature"
    ts.coordinates = "clat clon"
    ts[:] = np.stack([288 - 40 * np.sin(clat) ** 2 + 5 * np.cos(clon + k) for k in range(2)])
    nc.close()


def fvcom(path):
    """FVCOM-style: nv(three, nele) 1-based without start_index, lon/lat and x/y, data
    variables without mesh attributes, a sigma-layer axis."""
    nx, ny = 6, 5
    lon, lat = [], []
    for j in range(ny):
        for i in range(nx):
            lon.append(-70.5 + 0.05 * i + 0.01 * (j % 2))
            lat.append(41.2 + 0.04 * j)
    tris = []
    for j in range(ny - 1):
        for i in range(nx - 1):
            a = j * nx + i
            tris += [[a, a + 1, a + nx + 1], [a, a + nx + 1, a + nx]]
    tris = np.array(tris)
    nc = Dataset(path, "w", format="NETCDF3_64BIT_OFFSET")
    nc.title = "FVCOM-like"
    nc.source = "FVCOM_4.4"
    nc.createDimension("nele", len(tris))
    nc.createDimension("node", len(lon))
    nc.createDimension("three", 3)
    nc.createDimension("siglay", 2)
    nc.createDimension("time", None)
    m = nc.createVariable("fvcom_mesh", "i4")
    m.cf_role = "mesh_topology"
    m.topology_dimension = 2
    m.node_coordinates = "lon lat"
    m.face_coordinates = "lonc latc"
    m.face_node_connectivity = "nv"
    for name, dim, vals, units in [
        ("lon", "node", lon, "degrees_east"), ("lat", "node", lat, "degrees_north"),
        ("lonc", "nele", np.array(lon)[tris].mean(1), "degrees_east"),
        ("latc", "nele", np.array(lat)[tris].mean(1), "degrees_north"),
    ]:
        x = nc.createVariable(name, "f4", (dim,))
        x.units = units
        x[:] = vals
    nv = nc.createVariable("nv", "i4", ("three", "nele"))
    nv[:] = tris.T + 1
    t = nc.createVariable("time", "f4", ("time",))
    t.units = "days since 1858-11-17 00:00:00"
    t.format = "modified julian day (MJD)"
    t[:] = [58000.0, 58000.25]
    sl = nc.createVariable("siglay", "f4", ("siglay", "node"))
    sl.standard_name = "ocean_sigma_coordinate"
    sl[:] = np.array([[-0.25] * len(lon), [-0.75] * len(lon)])
    z = nc.createVariable("zeta", "f4", ("time", "node"))
    z.units = "meters"
    z.long_name = "Water Surface Elevation"
    z[:] = np.stack([np.array(lat) - 41.2 + 0.1 * k for k in range(2)])
    u = nc.createVariable("u", "f4", ("time", "siglay", "nele"))
    u.units = "meters s-1"
    clon = np.array(lon)[tris].mean(1)
    u[:] = np.stack([np.stack([clon + 70.5 + s + k for s in range(2)]) for k in range(2)])
    nc.close()


def big(path, level=8):
    """A global UGRID mesh from a subdivided icosahedron: 20 × 4^level faces (1.31M at
    level 8), node data, lon/lat degrees, NetCDF-4 with deflate."""
    v, f, lon, lat = icosphere(level)
    nc = Dataset(path, "w", format="NETCDF4")
    nc.Conventions = "CF-1.8 UGRID-1.0"
    nc.createDimension("node", len(v))
    nc.createDimension("face", len(f))
    nc.createDimension("max_nodes", 3)
    mesh_var(nc, "mesh", "node_lon", "node_lat", "face_nodes", face_dim="face")
    x = nc.createVariable("node_lon", "f8", ("node",), zlib=True)
    x.units = "degrees_east"
    x.standard_name = "longitude"
    x[:] = np.degrees(lon)
    x = nc.createVariable("node_lat", "f8", ("node",), zlib=True)
    x.units = "degrees_north"
    x.standard_name = "latitude"
    x[:] = np.degrees(lat)
    c = nc.createVariable("face_nodes", "i4", ("face", "max_nodes"), zlib=True)
    c.start_index = 0
    c[:] = f
    h = nc.createVariable("height", "f4", ("node",), zlib=True)
    h.mesh = "mesh"
    h.location = "node"
    h.units = "m"
    h[:] = 1000 * np.sin(3 * lon) * np.cos(2 * lat) + 500 * np.sin(5 * lat)
    k = nc.createVariable("cell_id", "f4", ("face",), zlib=True)
    k.mesh = "mesh"
    k.location = "face"
    k[:] = np.arange(len(f)) % 997
    nc.close()


def main():
    out = sys.argv[1]
    os.makedirs(out, exist_ok=True)
    tri_start0(os.path.join(out, "tri_start0.nc"))
    mixed_fill_start1(os.path.join(out, "mixed_fill_start1.nc"))
    fvcom(os.path.join(out, "fvcom_like.nc"))
    icon(os.path.join(out, "icon_grid.nc"), os.path.join(out, "icon_data.nc"))
    # The same global mesh small (1280 faces): node data on lon/lat for the GPU shots.
    big(os.path.join(out, "global_node.nc"), level=3)
    if "--big" in sys.argv:
        d = sys.argv[sys.argv.index("--big") + 1]
        os.makedirs(d, exist_ok=True)
        big(os.path.join(d, "global_1m.nc"))


if __name__ == "__main__":
    main()
