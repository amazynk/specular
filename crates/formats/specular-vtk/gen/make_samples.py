"""VTK sample files and what VTK itself reads from them (the oracle for specular-vtk).

    python -I make_samples.py OUT [--corpus | --bench]

Writes files with VTK's own writers (legacy 4.2 and 5.1, ASCII and binary; XML ascii,
inline binary and appended raw / base64 with no, zlib and LZ4 compression, UInt32 and UInt64
headers, both byte orders) and, beside each, `NAME.json` and `NAME.bin` describing what
VTK's readers get back:

* grids: dims, the points (structured) or coordinates (rectilinear), every point, cell and
  field array (values as float32, little-endian, in `NAME.bin` at `offset` floats);
* meshes: the boundary (vtkDataSetSurfaceFilter, linear faces) triangulated by
  vtkTriangleFilter: triangle count, total area and every triangle as its three corner
  positions sorted (9 floats), the list sorted.

Default: small fixtures for tests/data. `--corpus`: larger ones in every encoding. `--bench`:
a 256^3 float32 image in raw appended, zlib binary and ASCII, and a 1M-triangle .vtp.
"""

import json
import os
import sys

import numpy as np
import vtk
from vtk.util import numpy_support as ns

OUT = sys.argv[1]
MODE = sys.argv[2] if len(sys.argv) > 2 else "fixtures"
os.makedirs(OUT, exist_ok=True)
rng = np.random.default_rng(1234)

DTYPES = [np.int8, np.uint8, np.int16, np.uint16, np.int32, np.uint32, np.int64, np.uint64,
          np.float32, np.float64]


def vtk_array(a, name):
    v = ns.numpy_to_vtk(np.ascontiguousarray(a), deep=1)
    v.SetName(name)
    return v


def arrays_of(data):
    out = []
    for i in range(data.GetNumberOfArrays()):
        a = data.GetAbstractArray(i)
        if not isinstance(a, vtk.vtkDataArray):
            continue
        v = ns.vtk_to_numpy(a).astype(np.float32).reshape(-1)
        out.append((a.GetName() or "", a.GetNumberOfComponents(), a.GetNumberOfTuples(), v))
    return out


def surface(ds):
    f = vtk.vtkDataSetSurfaceFilter()
    f.SetInputData(ds)
    f.SetNonlinearSubdivisionLevel(0)
    t = vtk.vtkTriangleFilter()
    t.SetInputConnection(f.GetOutputPort())
    t.PassVertsOff()
    t.PassLinesOff()
    t.Update()
    pd = t.GetOutput()
    pts = ns.vtk_to_numpy(pd.GetPoints().GetData()).astype(np.float32) if pd.GetPoints() else np.zeros((0, 3), np.float32)
    tris = []
    area = 0.0
    polys = pd.GetPolys()
    ids = vtk.vtkIdList()
    polys.InitTraversal()
    while polys.GetNextCell(ids):
        if ids.GetNumberOfIds() != 3:
            continue
        p = [tuple(pts[ids.GetId(k)]) for k in range(3)]
        a, b, c = (np.array(x, np.float64) for x in p)
        area += 0.5 * np.linalg.norm(np.cross(b - a, c - a))
        tris.append(sorted(p))
    tris.sort()
    return tris, area


def describe(path):
    r = vtk.vtkDataSetReader() if path.endswith(".vtk") else vtk.vtkXMLGenericDataObjectReader()
    if path.endswith(".vtk"):
        r.ReadAllScalarsOn()
        r.ReadAllVectorsOn()
        r.ReadAllNormalsOn()
        r.ReadAllTensorsOn()
        r.ReadAllColorScalarsOn()
        r.ReadAllTCoordsOn()
        r.ReadAllFieldsOn()
    r.SetFileName(path)
    r.Update()
    ds = r.GetOutput()
    blob = []
    pos = [0]

    def put(v):
        v = np.asarray(v, np.float32).reshape(-1)
        o = pos[0]
        blob.append(v)
        pos[0] += v.size
        return o

    def section(data):
        return [{"name": n, "components": c, "tuples": t, "offset": put(v)}
                for (n, c, t, v) in arrays_of(data)]

    m = {"point_data": section(ds.GetPointData()), "cell_data": section(ds.GetCellData()),
         "field_data": section(ds.GetFieldData())}
    if isinstance(ds, (vtk.vtkImageData, vtk.vtkRectilinearGrid, vtk.vtkStructuredGrid)):
        m["kind"] = "grid"
        d = [0, 0, 0]
        ds.GetDimensions(d)
        m["dims"] = d
        if isinstance(ds, vtk.vtkImageData):
            m["origin"] = list(ds.GetPoint(0))
            m["spacing"] = list(ds.GetSpacing())
        if isinstance(ds, vtk.vtkRectilinearGrid):
            m["coords"] = [put(ns.vtk_to_numpy(c)) for c in
                           (ds.GetXCoordinates(), ds.GetYCoordinates(), ds.GetZCoordinates())]
        if isinstance(ds, vtk.vtkStructuredGrid):
            m["points"] = put(ns.vtk_to_numpy(ds.GetPoints().GetData()))
    else:
        m["kind"] = "mesh"
        m["points"] = ds.GetNumberOfPoints()
        m["cells"] = ds.GetNumberOfCells()
        tris, area = surface(ds)
        m["triangles"] = len(tris)
        m["area"] = area
        m["tri_offset"] = put(np.array(tris, np.float32).reshape(-1)) if tris else 0
    base = path.rsplit(".", 1)[0]
    with open(base + ".json", "w") as f:
        json.dump(m, f, indent=0)
    np.concatenate(blob or [np.zeros(0, np.float32)]).astype("<f4").tofile(base + ".bin")


# ---- sources ------------------------------------------------------------------------

def wavelet(n):
    s = vtk.vtkRTAnalyticSource()
    s.SetWholeExtent(0, n[0] - 1, 0, n[1] - 1, 0, n[2] - 1)
    s.Update()
    img = vtk.vtkImageData()
    img.DeepCopy(s.GetOutput())
    img.SetSpacing(0.5, 0.25, 2.0)
    img.SetOrigin(-1.0, 3.0, 0.5)
    return img


def typed_image(n):
    img = vtk.vtkImageData()
    img.SetDimensions(*n)
    img.SetSpacing(1.5, 1.0, 0.75)
    npts = n[0] * n[1] * n[2]
    ncells = max(n[0] - 1, 1) * max(n[1] - 1, 1) * max(n[2] - 1, 1)
    for dt in DTYPES:
        if np.issubdtype(dt, np.integer):
            info = np.iinfo(dt)
            lo, hi = max(info.min, -2**40), min(info.max, 2**40)
            v = rng.integers(lo, hi, size=npts, dtype=dt, endpoint=True)
        else:
            v = (rng.standard_normal(npts) * 1e3).astype(dt)
        img.GetPointData().AddArray(vtk_array(v, f"p_{np.dtype(dt).name}"))
    vec = rng.standard_normal((npts, 3)).astype(np.float32)
    img.GetPointData().SetVectors(vtk_array(vec, "velocity"))
    img.GetPointData().SetScalars(img.GetPointData().GetArray("p_float32"))
    img.GetCellData().AddArray(vtk_array(rng.integers(0, 100, size=ncells).astype(np.int32), "cell_id"))
    t = vtk.vtkDoubleArray()
    t.SetName("TIME")
    t.InsertNextValue(2.5)
    img.GetFieldData().AddArray(t)
    return img


def attributes(n, colours):
    """Every legacy attribute kind: colour scalars or scalars with a lookup table, normals,
    texture coordinates, tensors, global ids, a bit array."""
    img = vtk.vtkImageData()
    img.SetDimensions(*n)
    npts = n[0] * n[1] * n[2]
    pd = img.GetPointData()
    if colours:
        pd.SetScalars(vtk_array(rng.integers(0, 256, size=(npts, 3), dtype=np.uint8), "rgb"))
    else:
        s = vtk_array(rng.random(npts).astype(np.float32), "level")
        lut = vtk.vtkLookupTable()
        lut.SetNumberOfTableValues(4)
        lut.Build()
        s.SetLookupTable(lut)
        pd.SetScalars(s)
    nrm = rng.standard_normal((npts, 3)).astype(np.float32)
    pd.SetNormals(vtk_array(nrm, "normals"))
    pd.SetTCoords(vtk_array(rng.random((npts, 2)).astype(np.float32), "uv"))
    pd.SetTensors(vtk_array(rng.random((npts, 9)).astype(np.float64), "stress"))
    ids = vtk.vtkIdTypeArray()
    ids.SetName("gid")
    for i in range(npts):
        ids.InsertNextValue(1000 + i)
    pd.SetGlobalIds(ids)
    bits = vtk.vtkBitArray()
    bits.SetName("flags")
    for i in range(npts):
        bits.InsertNextValue(int(rng.integers(0, 2)))
    pd.AddArray(bits)
    return img


def rectilinear(n):
    g = vtk.vtkRectilinearGrid()
    g.SetDimensions(*n)
    for k, setter in enumerate((g.SetXCoordinates, g.SetYCoordinates, g.SetZCoordinates)):
        c = np.cumsum(rng.random(n[k]) + 0.1).astype(np.float64)
        setter(vtk_array(c, "c"))
    npts = n[0] * n[1] * n[2]
    g.GetPointData().SetScalars(vtk_array(rng.random(npts).astype(np.float32), "density"))
    return g


def structured(n):
    g = vtk.vtkStructuredGrid()
    g.SetDimensions(*n)
    pts = vtk.vtkPoints()
    for k in range(n[2]):
        for j in range(n[1]):
            for i in range(n[0]):
                pts.InsertNextPoint(i + 0.1 * np.sin(j), j + 0.2 * k, k + 0.05 * i)
    g.SetPoints(pts)
    npts = n[0] * n[1] * n[2]
    g.GetPointData().SetScalars(vtk_array(rng.random(npts).astype(np.float64), "pressure"))
    return g


def sphere(res, strips=False, extras=False):
    s = vtk.vtkSphereSource()
    s.SetThetaResolution(res)
    s.SetPhiResolution(res)
    s.Update()
    pd = s.GetOutput()
    if strips:
        st = vtk.vtkStripper()
        st.SetInputData(pd)
        st.Update()
        pd = st.GetOutput()
    out = vtk.vtkPolyData()
    out.DeepCopy(pd)
    n = out.GetNumberOfPoints()
    out.GetPointData().SetScalars(vtk_array(np.arange(n, dtype=np.float32) * 0.5, "elevation"))
    if extras:
        verts = vtk.vtkCellArray()
        for i in range(0, n, 3):
            verts.InsertNextCell(1, [i])
        out.SetVerts(verts)
        lines = vtk.vtkCellArray()
        lines.InsertNextCell(3, [0, 1, 2])
        out.SetLines(lines)
    out.GetCellData().AddArray(vtk_array(np.arange(out.GetNumberOfCells(), dtype=np.int32), "cid"))
    return out


def cube():
    s = vtk.vtkCubeSource()
    s.Update()
    out = vtk.vtkPolyData()
    out.DeepCopy(s.GetOutput())
    return out


def every_cell():
    """One unstructured grid with a cell of every supported type, side by side."""
    ug = vtk.vtkUnstructuredGrid()
    pts = vtk.vtkPoints()
    ug.SetPoints(pts)
    x0 = [0.0]

    def add(kind, coords, conn=None):
        base = pts.GetNumberOfPoints()
        for c in coords:
            pts.InsertNextPoint(c[0] + x0[0], c[1], c[2])
        ids = [base + i for i in range(len(coords))] if conn is None else [base + i for i in conn]
        ug.InsertNextCell(kind, len(ids), ids)
        x0[0] += 3.0

    sq = [(0, 0, 0), (1, 0, 0), (1, 1, 0), (0, 1, 0)]
    cubep = sq + [(x, y, 1) for (x, y, _) in sq]
    vox = [(0, 0, 0), (1, 0, 0), (0, 1, 0), (1, 1, 0), (0, 0, 1), (1, 0, 1), (0, 1, 1), (1, 1, 1)]
    tet = [(0, 0, 0), (1, 0, 0), (0, 1, 0), (0, 0, 1)]
    wed = [(0, 0, 0), (1, 0, 0), (0, 1, 0), (0, 0, 1), (1, 0, 1), (0, 1, 1)]
    pyr = sq + [(0.5, 0.5, 1)]

    def mid(a, b):
        return tuple((np.array(a) + np.array(b)) / 2)

    add(vtk.VTK_VERTEX, [(0, 0, 0)])
    add(vtk.VTK_LINE, [(0, 0, 0), (1, 1, 0)])
    add(vtk.VTK_POLY_LINE, [(0, 0, 0), (1, 1, 0), (2, 0, 0)])
    add(vtk.VTK_TRIANGLE, sq[:3])
    add(vtk.VTK_TRIANGLE_STRIP, [(0, 0, 0), (0, 1, 0), (1, 0, 0), (1, 1, 0), (2, 0, 0), (2, 1, 0)])
    add(vtk.VTK_POLYGON, [(0, 0, 0), (1, 0, 0), (1.5, 0.8, 0), (0.5, 1.5, 0), (-0.5, 0.8, 0)])
    add(vtk.VTK_PIXEL, [(0, 0, 0), (1, 0, 0), (0, 1, 0), (1, 1, 0)])
    add(vtk.VTK_QUAD, sq)
    add(vtk.VTK_TETRA, tet)
    add(vtk.VTK_VOXEL, vox)
    add(vtk.VTK_HEXAHEDRON, cubep)
    add(vtk.VTK_WEDGE, wed)
    add(vtk.VTK_PYRAMID, pyr)
    pent = [(np.cos(a), np.sin(a), 0) for a in np.linspace(0, 2 * np.pi, 6)[:5]]
    add(vtk.VTK_PENTAGONAL_PRISM, pent + [(x, y, 1) for (x, y, _) in pent])
    hexa = [(np.cos(a), np.sin(a), 0) for a in np.linspace(0, 2 * np.pi, 7)[:6]]
    add(vtk.VTK_HEXAGONAL_PRISM, hexa + [(x, y, 1) for (x, y, _) in hexa])
    t3 = sq[:3]
    add(vtk.VTK_QUADRATIC_TRIANGLE, t3 + [mid(t3[0], t3[1]), mid(t3[1], t3[2]), mid(t3[2], t3[0])])
    add(vtk.VTK_QUADRATIC_QUAD, sq + [mid(sq[i], sq[(i + 1) % 4]) for i in range(4)])
    tedges = [(0, 1), (1, 2), (2, 0), (0, 3), (1, 3), (2, 3)]
    add(vtk.VTK_QUADRATIC_TETRA, tet + [mid(tet[a], tet[b]) for a, b in tedges])
    hedges = [(0, 1), (1, 2), (2, 3), (3, 0), (4, 5), (5, 6), (6, 7), (7, 4), (0, 4), (1, 5), (2, 6), (3, 7)]
    add(vtk.VTK_QUADRATIC_HEXAHEDRON, cubep + [mid(cubep[a], cubep[b]) for a, b in hedges])
    wedges = [(0, 1), (1, 2), (2, 0), (3, 4), (4, 5), (5, 3), (0, 3), (1, 4), (2, 5)]
    add(vtk.VTK_QUADRATIC_WEDGE, wed + [mid(wed[a], wed[b]) for a, b in wedges])
    pedges = [(0, 1), (1, 2), (2, 3), (3, 0), (0, 4), (1, 4), (2, 4), (3, 4)]
    add(vtk.VTK_QUADRATIC_PYRAMID, pyr + [mid(pyr[a], pyr[b]) for a, b in pedges])
    # A polyhedron: a cube given by its faces.
    base = pts.GetNumberOfPoints()
    for c in cubep:
        pts.InsertNextPoint(c[0] + x0[0], c[1], c[2])
    faces = [[0, 3, 2, 1], [4, 5, 6, 7], [0, 1, 5, 4], [1, 2, 6, 5], [2, 3, 7, 6], [3, 0, 4, 7]]
    stream = vtk.vtkIdList()
    stream.InsertNextId(len(faces))
    for f in faces:
        stream.InsertNextId(len(f))
        for i in f:
            stream.InsertNextId(base + i)
    ug.InsertNextCell(vtk.VTK_POLYHEDRON, stream)
    n = ug.GetNumberOfCells()
    ug.GetCellData().SetScalars(vtk_array(np.arange(n, dtype=np.float32), "cell"))
    ug.GetPointData().AddArray(vtk_array(np.arange(pts.GetNumberOfPoints(), dtype=np.int64), "pid"))
    return ug


def voxels(n, tets=False):
    """An n×n×n block of cells sharing faces (voxels, or tetrahedra)."""
    img = vtk.vtkImageData()
    img.SetDimensions(n + 1, n + 1, n + 1)
    a = vtk.vtkAppendFilter()
    a.SetInputData(img)
    a.Update()
    ug = a.GetOutput()
    if tets:
        t = vtk.vtkDataSetTriangleFilter()
        t.SetInputData(ug)
        t.Update()
        ug = t.GetOutput()
    out = vtk.vtkUnstructuredGrid()
    out.DeepCopy(ug)
    out.GetPointData().SetScalars(vtk_array(rng.random(out.GetNumberOfPoints()).astype(np.float32), "temp"))
    return out


# ---- writers ------------------------------------------------------------------------

XML_WRITERS = {
    "vti": vtk.vtkXMLImageDataWriter,
    "vtr": vtk.vtkXMLRectilinearGridWriter,
    "vts": vtk.vtkXMLStructuredGridWriter,
    "vtu": vtk.vtkXMLUnstructuredGridWriter,
    "vtp": vtk.vtkXMLPolyDataWriter,
}


def ext_of(ds):
    if isinstance(ds, vtk.vtkImageData):
        return "vti"
    if isinstance(ds, vtk.vtkRectilinearGrid):
        return "vtr"
    if isinstance(ds, vtk.vtkStructuredGrid):
        return "vts"
    if isinstance(ds, vtk.vtkPolyData):
        return "vtp"
    return "vtu"


def xml(ds, name, mode="ascii", comp="none", h64=False, big=False, pieces=1):
    ext = ext_of(ds)
    w = XML_WRITERS[ext]()
    w.SetInputData(ds)
    if pieces > 1:
        w.SetNumberOfPieces(pieces)
    tag = f"{name}_{mode}_{comp}{'_h64' if h64 else ''}{'_be' if big else ''}{f'_p{pieces}' if pieces > 1 else ''}"
    path = os.path.join(OUT, f"{tag}.{ext}")
    w.SetFileName(path)
    if mode == "ascii":
        w.SetDataModeToAscii()
    elif mode == "binary":
        w.SetDataModeToBinary()
    else:
        w.SetDataModeToAppended()
        w.SetEncodeAppendedData(1 if mode == "b64" else 0)
    {"none": w.SetCompressorTypeToNone, "zlib": w.SetCompressorTypeToZLib,
     "lz4": w.SetCompressorTypeToLZ4, "lzma": w.SetCompressorTypeToLZMA}[comp]()
    if comp != "none":
        w.SetBlockSize(1024 if MODE != "bench" else 32768)
    if h64:
        w.SetHeaderTypeToUInt64()
    if big:
        w.SetByteOrderToBigEndian()
    w.Write()
    if comp != "lzma":
        describe(path)
    return path


def legacy(ds, name, version=51, binary=False):
    w = vtk.vtkDataSetWriter()
    w.SetInputData(ds)
    path = os.path.join(OUT, f"{name}_legacy{version}{'_bin' if binary else ''}.vtk")
    w.SetFileName(path)
    w.SetFileVersion(version)
    if binary:
        w.SetFileTypeToBinary()
    w.Write()
    describe(path)
    return path


def all_xml(ds, name):
    xml(ds, name)
    for mode in ("binary", "b64", "raw"):
        for comp in ("none", "zlib", "lz4"):
            for h64 in (False, True):
                for big in (False, True):
                    xml(ds, name, mode, comp, h64, big)


def all_legacy(ds, name):
    for v in (42, 51):
        for b in (False, True):
            legacy(ds, name, v, b)


if MODE == "fixtures":
    small = [
        ("wavelet", wavelet((5, 4, 3))),
        ("typed", typed_image((4, 3, 2))),
        ("rect", rectilinear((4, 3, 2))),
        ("colours", attributes((3, 3, 2), True)),
        ("lut", attributes((3, 2, 2), False)),
        ("sgrid", structured((3, 3, 2))),
        ("sphere", sphere(6, extras=True)),
        ("strips", sphere(6, strips=True)),
        ("cube", cube()),
        ("cells", every_cell()),
        ("block", voxels(2)),
    ]
    for name, ds in small:
        xml(ds, name)
        all_legacy(ds, name)
    by = dict(small)
    w, t, c = by["wavelet"], by["typed"], by["cells"]
    for (mode, comp, h64, big) in [("binary", "none", False, False), ("binary", "zlib", False, False),
                                   ("binary", "lz4", True, True), ("b64", "none", False, False),
                                   ("b64", "zlib", True, False), ("raw", "none", False, False),
                                   ("raw", "zlib", False, True), ("raw", "lz4", True, False)]:
        xml(t, "typed", mode, comp, h64, big)
    xml(w, "wavelet", "raw", "zlib", True, True)
    xml(c, "cells", "raw", "zlib")
    xml(by["sphere"], "sphere", "b64", "lz4", False, True)
    xml(by["sgrid"], "sgrid", "binary", "zlib")
    xml(by["rect"], "rect", "raw", "none", True, False)
    xml(w, "wavelet", "binary", "lzma")
    for name, ds in small:
        if name in ("wavelet", "rect", "sgrid", "sphere", "cells"):
            xml(ds, name, "raw", "none", pieces=3)
elif MODE == "corpus":
    sets = [
        ("wavelet", wavelet((32, 24, 16))),
        ("typed", typed_image((12, 10, 6))),
        ("rect", rectilinear((20, 15, 10))),
        ("colours", attributes((16, 12, 3), True)),
        ("lut", attributes((15, 9, 4), False)),
        ("sgrid", structured((12, 10, 8))),
        ("sphere", sphere(64, extras=True)),
        ("strips", sphere(48, strips=True)),
        ("cube", cube()),
        ("cells", every_cell()),
        ("block", voxels(10)),
        ("tets", voxels(6, tets=True)),
    ]
    for name, ds in sets:
        all_xml(ds, name)
        all_legacy(ds, name)
        xml(ds, name, "binary", "zlib", pieces=4)
else:
    big = wavelet((256, 256, 256))
    big.GetPointData().GetScalars().SetName("RTData")
    xml(big, "big", "raw", "none")
    xml(big, "big", "binary", "zlib")
    xml(big, "big", "ascii")
    s = vtk.vtkSphereSource()
    s.SetThetaResolution(1000)
    s.SetPhiResolution(502)
    s.Update()
    w = vtk.vtkXMLPolyDataWriter()
    w.SetInputData(s.GetOutput())
    w.SetFileName(os.path.join(OUT, "sphere1m.vtp"))
    w.SetDataModeToAppended()
    w.EncodeAppendedDataOff()
    w.SetCompressorTypeToNone()
    w.Write()
    for f in os.listdir(OUT):
        if f.endswith((".json", ".bin")):
            os.remove(os.path.join(OUT, f))
