"""Writes expected.tsv for the face locator and the values sampled at points, from
xugrid (face search through its cell tree, barycentric weights for node values) and
netCDF4 (the values).

    python gen/oracle.py DIR [FILE...]

Without FILEs every .nc in DIR. Rows: file, variable, axis positions (comma-joined,
"-" for none), x, y, face (-1 outside), value ("nan" masked, "-" when not compared: node
values inside non-triangle faces, which xugrid weights differently from a fan).

UGRID files are opened with xugrid.open_dataset (start_index, fill values, the
connectivity layout as the conventions say); FVCOM-style files whose `nv` has no
start_index are built explicitly 1-based, as FVCOM writes them; ICON files are built
from vertex_of_cell or the vertex bounds. Geographic meshes are compared away from the
antimeridian and the poles (xugrid works on the plane, the reader splits faces there).
"""

import os
import sys

import numpy as np
import xugrid as xu
from netCDF4 import Dataset

RNG = np.random.default_rng(20261012)


def topology(nc):
    for name, v in nc.variables.items():
        if getattr(v, "cf_role", None) == "mesh_topology" and getattr(v, "topology_dimension", 0) == 2:
            return name
    return None


def grid_of(path, nc):
    """(Ugrid2d, face dim, node dim, geographic, data var names)."""
    mesh = topology(nc)
    if mesh is not None:
        m = nc.variables[mesh]
        conn = nc.variables[m.face_node_connectivity]
        x, y = m.node_coordinates.split()
        face_dim = getattr(m, "face_dimension", None) or (
            conn.dimensions[1] if conn.shape[0] < conn.shape[1] else conn.dimensions[0]
        )
        # The connectivity as the conventions say: faces along face_dimension, indices from
        # start_index (FVCOM's nv: 1-based without one), _FillValue and anything below the
        # start (-1, MPAS-style 0) as no node. Built explicitly rather than through
        # xugrid.open_dataset, which refuses files with unrelated odd variables.
        conn.set_auto_mask(False)
        raw = np.asarray(conn[:]).astype("i8")
        if conn.dimensions[0] != face_dim:
            raw = raw.T
        nx_ = nc.variables[x][:].astype("f8")
        fill = getattr(conn, "_FillValue", None)
        ok = raw if fill is None else raw[raw != fill]
        ok = ok[ok >= 0]
        lo, hi = (ok.min(), ok.max()) if ok.size else (0, 0)
        if hasattr(conn, "start_index"):
            start = int(conn.start_index)
            # A stated start the indices contradict (0-based indices under start_index 1).
            if start == 1 and lo == 0 and hi <= len(nx_) - 1:
                start = 0
            elif start == 0 and lo >= 1 and hi == len(nx_):
                start = 1
        else:
            start = 1 if ok.size and lo >= 1 and hi == len(nx_) else 0
        bad = raw < start
        if fill is not None:
            bad |= raw == int(fill)
        raw = np.where(bad, -1, raw - start)
        grid = xu.Ugrid2d(nx_, nc.variables[y][:].astype("f8"), -1, raw)
        node_dim = nc.variables[x].dimensions[0]
        geo = "degree" in getattr(nc.variables[x], "units", "")
        return grid, face_dim, node_dim, geo
    # MPAS: verticesOnCell (nCells, maxEdges), 1-based with 0 as no vertex, radians.
    if "verticesOnCell" in nc.variables and "clon" not in nc.variables:
        voc = np.asarray(nc.variables["verticesOnCell"][:]).astype("i8") - 1
        lon = np.degrees(np.asarray(nc.variables["lonVertex"][:], "f8"))
        lat = np.degrees(np.asarray(nc.variables["latVertex"][:], "f8"))
        # Repeated vertices padding a row (some writers) count as no vertex.
        for row in voc:
            seen = row >= 0
            row[1:][(row[1:] == row[:-1]) & seen[1:]] = -1
        return xu.Ugrid2d(lon, lat, -1, voc), "nCells", "nVertices", True
    # ICON.
    if "vertex_of_cell" in nc.variables:
        voc = nc.variables["vertex_of_cell"][:].astype("i8").T - 1
        lon = np.degrees(nc.variables["vlon"][:])
        lat = np.degrees(nc.variables["vlat"][:])
        return xu.Ugrid2d(lon, lat, -1, voc), nc.variables["clon"].dimensions[0], nc.variables["vlon"].dimensions[0], True
    bl = np.degrees(nc.variables["clon_bnds"][:]).ravel()
    bt = np.degrees(nc.variables["clat_bnds"][:]).ravel()
    keys = {}
    nodes = []
    conn = []
    for i in range(len(bl)):
        k = (bl[i], bt[i])
        if k not in keys:
            keys[k] = len(nodes)
            nodes.append(k)
        conn.append(keys[k])
    conn = np.array(conn).reshape(nc.variables["clon_bnds"].shape)
    nodes = np.array(nodes)
    return xu.Ugrid2d(nodes[:, 0], nodes[:, 1], -1, conn), nc.variables["clon"].dimensions[0], None, True


def main():
    d = sys.argv[1]
    files = sys.argv[2:] or sorted(f for f in os.listdir(d) if f.endswith(".nc"))
    rows = []
    for f in files:
        path = os.path.join(d, f)
        nc = Dataset(path)
        nc.set_auto_mask(True)
        try:
            grid, face_dim, node_dim, geo = grid_of(path, nc)
        except Exception as e:  # noqa: BLE001 - reported and skipped
            print(f"{f}: skipped ({type(e).__name__}: {e})", file=sys.stderr)
            nc.close()
            continue
        fnc = grid.face_node_connectivity
        nx, ny = grid.node_x, grid.node_y
        nodes_per_face = (fnc >= 0).sum(1)
        span = np.where(fnc >= 0, nx[np.maximum(fnc, 0)], np.nan)
        lon_span = np.nanmax(span, 1) - np.nanmin(span, 1)
        x0, x1, y0, y1 = nx.min(), nx.max(), ny.min(), ny.max()
        global_mesh = geo and x1 - x0 > 300
        if global_mesh:
            x0, x1, y0, y1 = -175.0, 175.0, -75.0, 75.0
        dx, dy = (x1 - x0) * 0.05, (y1 - y0) * 0.05
        pts = np.stack([RNG.uniform(x0 - dx, x1 + dx, 80), RNG.uniform(y0 - dy, y1 + dy, 80)], 1)
        cent = grid.face_coordinates[RNG.choice(grid.n_face, min(20, grid.n_face), replace=False)]
        pts = np.concatenate([pts, cent])
        faces = grid.locate_points(pts)
        bary_face, weights = grid.compute_barycentric_weights(pts)
        keep = np.ones(len(pts), bool)
        if geo:
            keep &= ~((faces >= 0) & (lon_span[np.maximum(faces, 0)] > 180))
            keep &= np.abs(pts[:, 1]) < 80
            if global_mesh:
                # A global mesh covers every point: a miss is xugrid's planar view of a
                # face across the antimeridian.
                keep &= faces >= 0
        # The face alone (variable "-"), so grid-only files are checked too.
        for i, (px, py) in enumerate(pts):
            if keep[i]:
                rows.append(f"{f}\t-\t-\t{float(px)!r}\t{float(py)!r}\t{int(faces[i])}\t-")
        # Whatever a topology names (connectivity, coordinates) is not data.
        named = {
            n
            for t in nc.variables.values()
            if getattr(t, "cf_role", None) == "mesh_topology"
            for k in t.ncattrs()
            if isinstance(t.getncattr(k), str)
            for n in t.getncattr(k).split()
        }
        for name, v in nc.variables.items():
            dims = v.dimensions
            if name in named:
                continue
            if not np.issubdtype(v.dtype, np.number) or name in ("clon", "clat"):
                continue
            if face_dim in dims and (node_dim is None or node_dim not in dims):
                loc, k = "face", dims.index(face_dim)
            elif node_dim is not None and node_dim in dims and face_dim not in dims:
                loc, k = "node", dims.index(node_dim)
            else:
                continue
            if len(dims) == 1 and name == dims[0]:
                continue
            if getattr(v, "cf_role", None) or name in ("node_lon", "node_lat"):
                continue
            if getattr(v, "standard_name", "") in ("longitude", "latitude") or name in (
                "Mesh2_node_x", "Mesh2_node_y", "mesh2d_node_x", "mesh2d_node_y",
                "lon", "lat", "lonc", "latc", "siglay", "vlon", "vlat",
                "nv", "vertex_of_cell", "clon_bnds", "clat_bnds",
            ):
                continue
            axes = [i for i in range(len(dims)) if i != k and v.shape[i] > 1]
            # The first and last position of each axis.
            combos = [[]]
            for a in axes:
                combos = [c + [p] for c in combos for p in sorted({0, v.shape[a] - 1})]
            for combo in combos:
                sl = [0] * len(dims)
                for a, p in zip(axes, combo):
                    sl[a] = p
                sl[k] = slice(None)
                vals = np.ma.filled(np.ma.asarray(v[tuple(sl)]).astype("f8"), np.nan)
                for i, (px, py) in enumerate(pts):
                    if not keep[i]:
                        continue
                    face = int(faces[i])
                    if face < 0:
                        value = "nan"
                    elif loc == "face":
                        value = repr(float(vals[face]))
                    elif nodes_per_face[face] == 3 and bary_face[i] == face:
                        nodes = fnc[face][:3]
                        # The cell tree orders clockwise faces counter-clockwise (reversed)
                        # and gives the weights in that order.
                        (ax, ay), (bx, by), (cx, cy) = zip(nx[nodes], ny[nodes])
                        if (bx - ax) * (cy - ay) - (by - ay) * (cx - ax) < 0:
                            nodes = nodes[::-1]
                        value = repr(float(np.dot(weights[i][:3], vals[nodes])))
                    else:
                        value = "-"
                    idx = ",".join(str(c) for c in combo) or "-"
                    rows.append(f"{f}\t{name}\t{idx}\t{float(px)!r}\t{float(py)!r}\t{face}\t{value}")
        nc.close()
    with open(os.path.join(d, "expected.tsv"), "w") as out:
        out.write("# file\tvariable\tindex\tx\ty\tface\tvalue\n")
        out.write("\n".join(rows) + "\n")
    print(len(rows), "rows")


if __name__ == "__main__":
    main()
