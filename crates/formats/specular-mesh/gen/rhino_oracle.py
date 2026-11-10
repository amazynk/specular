#!/usr/bin/env python3
"""Rhino (.3dm) test files and oracle facts, through rhino3dm (McNeel's openNURBS binding).

Run with `python -I` from the venv described in tools/corpus/fetch-models-extra.sh:

    python -I crates/formats/specular-mesh/gen/rhino_oracle.py tools/corpus/models-extra

1. Writes generated models into OUT/rhino/gen/: coloured meshes saved as Rhino 5, 6, 7 and
   8 archives, a block placed twice with transforms (and nested), hidden objects and a
   hidden layer, a B-rep without render meshes (to be refused) and a large mesh (timing).
2. Writes OUT/rhino/oracle.tsv, one row per file and fact (relpath, key, value) for every
   .3dm under OUT/rhino:
     tris   triangles drawn: mesh faces (quads count two) of the visible meshes, B-rep and
            extrusion render meshes, block instances expanded with their transforms
     verts  vertices of those meshes
     min, max  bounds of the vertices the faces use (file units)
     error  when rhino3dm cannot read the file
"""
import math
import os
import sys

import rhino3dm as r


def write_generated(out):
    os.makedirs(out, exist_ok=True)

    def mesh(points, faces, colours=None):
        m = r.Mesh()
        for p in points:
            m.Vertices.Add(*p)
        for f in faces:
            m.Faces.AddFace(*f)
        if colours:
            for c in colours:
                m.VertexColors.Add(*c)
        m.Normals.ComputeNormals()
        return m

    square = [(0, 0, 0), (1, 0, 0), (1, 1, 0), (0, 1, 0)]
    for v in (5, 6, 7, 8):
        f = r.File3dm()
        f.Settings.ModelUnitSystem = r.UnitSystem.Millimeters
        red = r.Layer()
        red.Name = "Red"
        red.Color = (255, 0, 0, 255)
        li = f.Layers.Add(red)
        a = r.ObjectAttributes()
        a.LayerIndex = li
        f.Objects.AddMesh(mesh(square, [(0, 1, 2, 3)]), a)
        a2 = r.ObjectAttributes()
        a2.ColorSource = r.ObjectColorSource.ColorFromObject
        a2.ObjectColor = (0, 160, 0, 255)
        f.Objects.AddMesh(
            mesh([(2, 0, 0), (3, 0, 0), (3, 1, 2)], [(0, 1, 2)],
                 [(255, 0, 0), (0, 255, 0), (0, 0, 255)]), a2)
        f.Write(os.path.join(out, "meshes_v%d.3dm" % v), v)

    # A block: a mesh defined once, placed twice (one rotated), and a block of blocks.
    f = r.File3dm()
    base = r.Point3d(0, 0, 0)
    tri = mesh([(0, 0, 0), (1, 0, 0), (0, 1, 1)], [(0, 1, 2)])
    idef = f.InstanceDefinitions.Add("Leaf", "", "", "", base, (tri,), (r.ObjectAttributes(),))
    leaf = f.InstanceDefinitions.FindIndex(idef)
    f.Objects.AddInstanceObject(r.InstanceReference(leaf.Id, r.Transform.Translation(5, 0, 0)))
    rot = r.Transform.Rotation(math.pi / 2, r.Vector3d(0, 0, 1), r.Point3d(0, 0, 0))
    f.Objects.AddInstanceObject(r.InstanceReference(leaf.Id, rot))
    inner = r.InstanceReference(leaf.Id, r.Transform.Translation(0, 0, 3))
    pair = f.InstanceDefinitions.Add("Pair", "", "", "", base, (inner,), (r.ObjectAttributes(),))
    pair_def = f.InstanceDefinitions.FindIndex(pair)
    f.Objects.AddInstanceObject(
        r.InstanceReference(pair_def.Id, r.Transform.Translation(-4, -4, 0)))
    f.Write(os.path.join(out, "blocks_v8.3dm"), 8)

    # Hidden things: a hidden object, an object on a hidden layer, one shown.
    f = r.File3dm()
    off = r.Layer()
    off.Name = "Off"
    off.Visible = False
    li = f.Layers.Add(off)
    a = r.ObjectAttributes()
    a.Visible = False
    f.Objects.AddMesh(mesh(square, [(0, 1, 2, 3)]), a)
    a = r.ObjectAttributes()
    a.LayerIndex = li
    f.Objects.AddMesh(mesh([(9, 9, 9), (10, 9, 9), (10, 10, 9)], [(0, 1, 2)]), a)
    f.Objects.AddMesh(mesh([(0, 0, 5), (1, 0, 5), (1, 1, 6)], [(0, 1, 2)]))
    f.Write(os.path.join(out, "hidden_v7.3dm"), 7)

    # NURBS only, no render meshes: refused.
    f = r.File3dm()
    box = r.BoundingBox(r.Point3d(0, 0, 0), r.Point3d(1, 2, 3))
    f.Objects.AddBrep(r.Brep.CreateFromBoundingBox(box))
    f.Write(os.path.join(out, "nurbs_only_v8.3dm"), 8)

    # A large mesh: a 600 × 600 grid of quads on a wave.
    n = 600
    m = r.Mesh()
    for j in range(n + 1):
        for i in range(n + 1):
            m.Vertices.Add(i * 0.1, j * 0.1, math.sin(i * 0.05) * math.cos(j * 0.05))
    for j in range(n):
        for i in range(n):
            a = j * (n + 1) + i
            m.Faces.AddFace(a, a + 1, a + n + 2, a + n + 1)
    m.Normals.ComputeNormals()
    f = r.File3dm()
    f.Objects.AddMesh(m)
    f.Write(os.path.join(out, "large_grid_v8.3dm"), 8)


def meshes_of(f, obj, xf, depth, out):
    g = obj.Geometry
    if isinstance(g, r.Mesh):
        out.append((g, xf))
    elif isinstance(g, r.Brep):
        for face in g.Faces:
            m = face.GetMesh(r.MeshType.Render)
            if m is not None:
                out.append((m, xf))
    elif isinstance(g, r.Extrusion):
        m = g.GetMesh(r.MeshType.Render)
        if m is not None:
            out.append((m, xf))
    elif isinstance(g, r.InstanceReference) and depth < 16:
        idef = f.InstanceDefinitions.FindId(g.ParentIdefId)
        if idef is None:
            return
        inner = r.Transform.Multiply(xf, g.Xform)
        for oid in idef.GetObjectIds():
            o = f.Objects.FindId(oid)
            if o is not None and o.Attributes.Visible:
                meshes_of(f, o, inner, depth + 1, out)


def shown(f, o):
    a = o.Attributes
    if not a.Visible or a.IsInstanceDefinitionObject:
        return False
    if a.Mode == r.ObjectMode.Hidden:
        return False
    if 0 <= a.LayerIndex < len(f.Layers):
        return f.Layers[a.LayerIndex].Visible
    return True


def facts(path):
    f = r.File3dm.Read(path)
    if f is None:
        return [("error", "unreadable")]
    found = []
    for o in f.Objects:
        if shown(f, o):
            meshes_of(f, o, r.Transform.Identity(), 0, found)
    tris = verts = 0
    lo = [math.inf] * 3
    hi = [-math.inf] * 3
    for m, xf in found:
        if not xf.IsIdentity:
            m = m.Duplicate()
            m.Transform(xf)
        vs = m.Vertices
        nv = len(vs)
        used = [False] * nv
        for k in range(len(m.Faces)):
            q = m.Faces[k]
            if not all(0 <= i < nv for i in q):
                continue
            tris += 1 if q[2] == q[3] else 2
            for i in q:
                used[i] = True
        verts += nv
        for i in range(nv):
            if used[i]:
                p = vs[i]
                for c, x in enumerate((p.X, p.Y, p.Z)):
                    lo[c] = min(lo[c], x)
                    hi[c] = max(hi[c], x)
    rows = [("tris", str(tris)), ("verts", str(verts))]
    if tris:
        rows.append(("min", ",".join("%.9g" % x for x in lo)))
        rows.append(("max", ",".join("%.9g" % x for x in hi)))
    return rows


def main():
    root = os.path.join(sys.argv[1], "rhino")
    write_generated(os.path.join(root, "gen"))
    rows = []
    for d, _, names in sorted(os.walk(root)):
        for n in sorted(names):
            if n.lower().endswith(".3dm"):
                p = os.path.join(d, n)
                rel = os.path.relpath(p, root)
                try:
                    for k, v in facts(p):
                        rows.append((rel, k, v))
                except Exception as e:  # an oracle failure is a fact too
                    rows.append((rel, "error", str(e).replace("\t", " ")[:200]))
    with open(os.path.join(root, "oracle.tsv"), "w") as o:
        o.write("relpath\tkey\tvalue\n")
        for row in rows:
            o.write("\t".join(row) + "\n")
    print("rhino oracle: %d files" % len({r_[0] for r_ in rows}))


if __name__ == "__main__":
    main()
